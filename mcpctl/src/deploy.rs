// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Pushing the declared servers into the live host configs under `$HOME`.
//!
//! This is the step the repository never had. A change could land correctly in all 13
//! templates and reach no host at all, because nothing reads the templates — they are a
//! reference, not a source. `perplexity` sat in every template and in only 7 of 15 live
//! configs for exactly that reason.
//!
//! Four properties matter more than convenience here, because these are files the user
//! and their tools own:
//!
//! 1. **Nothing else in the file moves.** Only the MCP block is replaced, by byte range.
//! 2. **A working key is never downgraded to a placeholder.** Real values already in the
//!    live file are carried into the rewritten block.
//! 3. **Unknown servers are never dropped silently.** They are reported, and removed
//!    only after an explicit yes.
//! 4. **A file is never left corrupt.** The result is re-parsed before it is written,
//!    and the write itself is a rename over a fully written temporary.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::{Map, Value, json};

use crate::dialect::{Format, HOSTS, Host, Strictness};
use crate::manifest::{HostConfig, Manifest};
use crate::runtime::{ExitCode, Failure, Profile};
use crate::{dialect, emit, render, splice};

/// Whether two renderings of a host config differ in CONTENT rather than
/// merely in layout.
///
/// This replaces a byte comparison, which made any host that pretty-prints
/// differently from `mcpctl` drift permanently. Claude Code rewrites
/// `~/.claude.json` in its own formatting when it exits; the reserialization
/// was reported as drift; a deploy reformatted the file; the host reformatted
/// it back. No deploy could win that, and the probe cried wolf after every
/// session.
///
/// The observed case: `dirty: true` with `added`, `updated` and `strays` all
/// empty — 296 bytes and 37 lines of whitespace across a 244 KB file, with not
/// one server, key, or value different. The per-server comparison in [`plan`]
/// was already structural and correctly found nothing; only this file-level
/// flag disagreed.
///
/// Key ORDER is deliberately not a difference either. [`plan`] preserves
/// position so a human diff stays small, but a reordered map is the same
/// configuration, and rewriting a file to sort it is the same wasted write.
fn content_differs(before: &str, after: &str, format: Format) -> bool {
    match (
        dialect::parse(format, before, Strictness::Lenient),
        dialect::parse(format, after, Strictness::Lenient),
    ) {
        (Ok(old), Ok(new)) => old != new,
        // Unparseable on either side is not a licence to call it clean: fall
        // back to the byte comparison, so a malformed file still reports drift
        // rather than being silently skipped.
        _ => before != after,
    }
}

/// What deploying one file would do.
#[derive(Debug)]
struct Change {
    /// Host the file belongs to.
    host: &'static str,
    /// Dialect of the file, absent for companion files that hold no server map.
    dialect: Option<&'static Host>,
    /// Absolute path of the live file.
    path: PathBuf,
    /// Contents as found, needed to re-splice if strays are pruned.
    original: String,
    /// Merged server map, before any pruning.
    merged: Map<String, Value>,
    /// Servers not previously present.
    added: Vec<String>,
    /// Servers whose entry changes.
    updated: Vec<String>,
    /// Servers present that the manifest does not declare.
    strays: Vec<String>,
    /// Secret keys that had no value to carry over and were therefore omitted.
    needs_key: Vec<String>,
    /// Whether strays may be offered for removal.
    prunable: bool,
    /// Full contents to write.
    new_text: String,
    /// Whether the file would change at all.
    dirty: bool,
}

/// How a deploy should behave.
#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    /// Report what would change without writing anything.
    pub dry_run: bool,
    /// Skip every confirmation prompt.
    pub yes: bool,
    /// Deploy even while a host that owns its config is running.
    pub force: bool,
}

/// Everything the planning pass discovered.
#[derive(Debug, Default)]
struct Survey {
    /// Files that can be deployed to.
    changes: Vec<Change>,
    /// Files that are absent.
    skipped: Vec<String>,
    /// Hosts held back because they are running.
    blocked: Vec<String>,
}

/// Deploys the manifest into every live config.
pub fn run(
    repo: &Path,
    options: Options,
    host_filter: Option<&str>,
    profile: &Profile,
) -> Result<ExitCode, Failure> {
    execute(repo, options, host_filter, profile).map_err(|error| {
        Failure::new(
            "DEPLOY_FAILED",
            ExitCode::Failed,
            format!("{error:#}"),
            "mcpctl deploy --dry-run --json",
        )
    })
}

/// Plans and applies the deploy.
fn execute(
    repo: &Path,
    options: Options,
    host_filter: Option<&str>,
    profile: &Profile,
) -> Result<ExitCode> {
    let manifest = Manifest::load(repo)?;
    let home = home_dir()?;

    // Writing is opt-in. Without a terminal to confirm at, and without an explicit
    // --yes, the only safe reading of the request is "show me what you would do" — this
    // is what keeps an agent-driven or CI run from rewriting a developer's configs.
    let interactive = profile.interactive;
    let dry_run = options.dry_run || (!options.yes && !interactive);

    let survey = survey(&manifest, &home, host_filter, options.force)?;
    report(&survey, dry_run, profile);

    if dry_run {
        if !profile.json {
            println!("\nnothing written (dry run). Re-run with --yes from a terminal to apply.");
        }
        return Ok(ExitCode::Ok);
    }

    let written = apply(&survey.changes, &home, interactive)?;
    if !profile.json {
        println!("\n{written} file(s) written");
        println!("Restart every host — a failed MCP server is not retried in-process.");
    }
    Ok(ExitCode::Ok)
}

/// Works out what deploying would do, without touching anything.
fn survey(
    manifest: &Manifest,
    home: &Path,
    host_filter: Option<&str>,
    force: bool,
) -> Result<Survey> {
    let placeholders = placeholder_index(manifest);
    let mut survey = Survey::default();

    for host in HOSTS {
        if host_filter.is_some_and(|wanted| wanted != host.name) {
            continue;
        }
        let config = manifest
            .host(host.name)
            .with_context(|| format!("manifest has no entry for `{}`", host.name))?;

        if let Some(process) = &config.guard_process
            && !force
            && process_running(process)
        {
            survey.blocked.push(format!(
                "{} — `{process}` is running and rewrites its own config; exit it first",
                host.name
            ));
            continue;
        }

        for path in config.live_paths(home) {
            if !path.exists() {
                survey
                    .skipped
                    .push(format!("{} — {} not present", host.name, path.display()));
                continue;
            }
            let change = plan_servers(&path, host, manifest, config, &placeholders)
                .with_context(|| path.display().to_string())?;
            survey.changes.push(change);
        }

        for (live, kind) in [
            (&config.companion_enablement_live, Companion::Enablement),
            (&config.companion_disabled_live, Companion::Disabled),
        ] {
            let Some(live) = live else { continue };
            let path = crate::manifest::expand_home(live, home);
            if !path.exists() {
                survey
                    .skipped
                    .push(format!("{} — {} not present", host.name, path.display()));
                continue;
            }
            let change = plan_companion(&path, host, manifest, kind)
                .with_context(|| path.display().to_string())?;
            survey.changes.push(change);
        }
    }
    Ok(survey)
}

/// Writes the planned changes, prompting when there is a terminal to prompt at.
fn apply(changes: &[Change], home: &Path, interactive: bool) -> Result<usize> {
    let backup_root = home.join(".mcp-backup").join(timestamp());
    let mut written = 0usize;

    for change in changes {
        let prunable_strays = interactive && change.prunable && !change.strays.is_empty();
        if !change.dirty && !prunable_strays {
            continue;
        }
        if interactive && !confirm(&format!("Apply to {}? [Y/n] ", change.path.display()), true)? {
            continue;
        }

        let mut text = change.new_text.clone();
        if prunable_strays && let Some(host) = change.dialect {
            let mut kept = change.merged.clone();
            let mut pruned = false;
            for stray in &change.strays {
                if confirm(
                    &format!("  remove `{stray}`, not in the manifest? [y/N] "),
                    false,
                )? {
                    // `shift_remove` rather than `remove`: with `preserve_order` the
                    // latter is a swap-remove and would shuffle the whole block.
                    kept.shift_remove(stray);
                    pruned = true;
                }
            }
            if pruned {
                let managed: Vec<String> = kept
                    .keys()
                    .filter(|name| !change.strays.contains(name))
                    .cloned()
                    .collect();
                text = rewrite(&change.original, host, &kept)?;
                verify(&text, host, &change.path, &managed)?;
            }
        }

        if text == change.original {
            continue;
        }
        backup(&change.path, &backup_root)?;
        write_atomically(&change.path, &text)?;
        written += 1;
    }

    if written > 0 {
        println!("backups in {}", backup_root.display());
    }
    Ok(written)
}

/// Which companion file is being planned.
#[derive(Debug, Clone, Copy)]
enum Companion {
    /// Gemini's per-server enablement map.
    Enablement,
    /// Claude Code's list of servers to leave switched off.
    Disabled,
}

/// Builds the new contents of one live server config.
fn plan_servers(
    path: &Path,
    host: &'static Host,
    manifest: &Manifest,
    config: &HostConfig,
    placeholders: &BTreeMap<String, String>,
) -> Result<Change> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read `{}`", path.display()))?;
    let root = dialect::parse(host.format, &text, Strictness::Lenient)?;
    let live = splice::servers_of(&root, host.wrapper)?;

    let mut desired = Map::new();
    for resolved in manifest.resolve_for(host.name) {
        desired.insert(
            resolved.name.clone(),
            render::server_entry(&resolved, host)?,
        );
    }

    let mut merged = Map::new();
    let mut added = Vec::new();
    let mut updated = Vec::new();
    let mut strays = Vec::new();

    // Existing entries keep their position, so the diff a user sees is only the change.
    let mut needs_key: Vec<String> = Vec::new();
    for (name, current) in &live {
        if let Some(entry) = desired.get(name) {
            let (entry, unresolved) =
                resolve_secrets(entry, Some(current), host, placeholders, &BTreeMap::new());
            if &entry != current {
                updated.push(name.clone());
            }
            needs_key.extend(unresolved);
            merged.insert(name.clone(), entry);
        } else {
            strays.push(name.clone());
            merged.insert(name.clone(), current.clone());
        }
    }
    for (name, entry) in &desired {
        if !merged.contains_key(name) {
            let (entry, unresolved) =
                resolve_secrets(entry, None, host, placeholders, &BTreeMap::new());
            needs_key.extend(unresolved);
            added.push(name.clone());
            merged.insert(name.clone(), entry);
        }
    }
    needs_key.sort_unstable();
    needs_key.dedup();

    let managed: Vec<String> = desired.keys().cloned().collect();
    let new_text = rewrite(&text, host, &merged)?;
    verify(&new_text, host, path, &managed)?;

    let dirty = content_differs(&text, &new_text, host.format);

    Ok(Change {
        host: host.name,
        dialect: Some(host),
        path: path.to_path_buf(),
        dirty,
        original: text,
        merged,
        added,
        updated,
        strays,
        needs_key,
        prunable: config.prune_strays,
        new_text,
    })
}

/// Builds the new contents of a companion enablement or disabled-list file.
fn plan_companion(
    path: &Path,
    host: &'static Host,
    manifest: &Manifest,
    kind: Companion,
) -> Result<Change> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read `{}`", path.display()))?;
    let root = dialect::parse(Format::Json, &text, Strictness::Lenient)?;
    let servers = manifest.resolve_for(host.name);
    let indent = splice::detect_indent(&text);

    let new_text = match kind {
        Companion::Enablement => {
            let mut merged = root
                .as_object()
                .cloned()
                .context("enablement file is not an object")?;
            for server in &servers {
                merged.insert(server.name.clone(), json!({ "enabled": server.enabled }));
            }
            emit::json(&Value::Object(merged), indent)
        }
        Companion::Disabled => {
            let mut merged = root
                .as_object()
                .cloned()
                .context("settings file is not an object")?;
            let disabled: Vec<&str> = servers
                .iter()
                .filter(|server| !server.enabled)
                .map(|server| server.name.as_str())
                .collect();
            merged.insert("disabledMcpjsonServers".to_owned(), json!(disabled));
            emit::json(&Value::Object(merged), indent)
        }
    };

    // Companion files are always JSON, and get the same structural comparison
    // as the server maps: reformatting one is not a change worth writing.
    let dirty = content_differs(&text, &new_text, Format::Json);

    let updated = if dirty {
        vec![path.file_name().map_or_else(
            || "companion".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        )]
    } else {
        Vec::new()
    };

    Ok(Change {
        host: host.name,
        dialect: None,
        path: path.to_path_buf(),
        dirty,
        original: text,
        merged: Map::new(),
        added: Vec::new(),
        updated,
        strays: Vec::new(),
        needs_key: Vec::new(),
        prunable: false,
        new_text,
    })
}

/// Copies a rendered entry, keeping any real secret the live file already holds and
/// dropping any placeholder that could not be resolved.
///
/// Two failure modes are avoided here. Writing the rendered entry as-is would overwrite
/// working keys with `YOUR_..._API_KEY` and break auth until `fill-keys` ran again. But
/// keeping the placeholder when there is nothing to carry over is worse still: a literal
/// `YOUR_CONTEXT7_API_KEY` is sent as a credential and *rejected*, whereas omitting the
/// header entirely leaves the server working anonymously. Placeholders belong in
/// templates, which are documentation; a live config either has a real key or no key.
///
/// Returns the entry and the names of any secrets that were dropped, so the caller can
/// tell the user which servers still need a key.
pub fn resolve_secrets(
    rendered: &Value,
    live: Option<&Value>,
    host: &Host,
    placeholders: &BTreeMap<String, String>,
    supplied: &BTreeMap<String, String>,
) -> (Value, Vec<String>) {
    let mut entry = rendered.clone();
    let mut unresolved = Vec::new();

    for field in [host.headers_field, host.env_field] {
        let existing = live
            .and_then(|value| value.get(field))
            .and_then(Value::as_object)
            .cloned();
        let Some(target) = entry.get_mut(field).and_then(Value::as_object_mut) else {
            continue;
        };

        let mut drop_keys = Vec::new();
        for (key, value) in target.iter_mut() {
            let Some(text) = value.as_str() else { continue };
            // The placeholder may be the whole value (`YOUR_BRAVE_API_KEY` as an
            // environment variable) or embedded in one (`Bearer YOUR_CONTEXT7_API_KEY`
            // as an Authorization header), so match on containment and substitute in
            // place rather than replacing the value wholesale.
            let Some((token, env_name)) = placeholders
                .iter()
                .find(|(token, _)| text.contains(token.as_str()))
            else {
                continue;
            };
            let text = text.to_owned();

            // A value supplied on this run wins, so `fill-keys` can rotate a key that is
            // already in place. Otherwise whatever the live config holds is kept.
            if let Some(fresh) = supplied.get(env_name) {
                *value = json!(text.replace(token.as_str(), fresh));
            } else if let Some(current) = existing
                .as_ref()
                .and_then(|map| map.get(key))
                .and_then(Value::as_str)
                .filter(|current| {
                    !contains_placeholder(current, placeholders) && !current.is_empty()
                })
            {
                *value = json!(current);
            } else {
                drop_keys.push(key.clone());
                unresolved.push(key.clone());
            }
        }
        for key in drop_keys {
            target.shift_remove(&key);
        }
        if target.is_empty() {
            entry
                .as_object_mut()
                .expect("entry is an object")
                .shift_remove(field);
        }
    }

    (entry, unresolved)
}

/// Produces the full new file contents with only the MCP block replaced.
pub fn rewrite(text: &str, host: &Host, servers: &Map<String, Value>) -> Result<String> {
    match host.format {
        Format::Toml => {
            let wrapper = host.wrapper.context("a TOML host needs a wrapper table")?;
            splice::replace_toml_block(text, wrapper, servers)
        }
        Format::Yaml => {
            let wrapper = host.wrapper.context("a YAML host needs a wrapper key")?;
            let block = splice::locate_yaml(text, wrapper)?;
            Ok(splice::apply(
                text,
                block,
                &emit::yaml(wrapper, servers, None),
            ))
        }
        Format::Json | Format::Jsonc => {
            let indent = splice::detect_indent(text);
            let block = splice::locate_json(text, host.wrapper)?;
            let fragment =
                emit::json_fragment(&Value::Object(servers.clone()), indent, block.depth);
            Ok(splice::apply(text, block, &fragment))
        }
    }
}

/// Re-parses rewritten contents, so a corrupt result is never written.
///
/// Only the servers this repository manages are held to the dialect. A live config also
/// contains entries belonging to the host itself — goose ships 16 `builtin` and
/// `platform` extensions in the same block, with neither `cmd` nor `uri` — and those are
/// not ours to validate, only to preserve byte-for-byte.
pub fn verify(text: &str, host: &Host, path: &Path, managed: &[String]) -> Result<()> {
    let root = dialect::parse(host.format, text, Strictness::Lenient)
        .with_context(|| format!("rewriting `{}` produced invalid syntax", path.display()))?;
    let servers = splice::servers_of(&root, host.wrapper).with_context(|| {
        format!(
            "rewriting `{}` produced an unreadable block",
            path.display()
        )
    })?;
    for name in managed {
        let entry = servers
            .get(name)
            .with_context(|| format!("rewriting `{}` lost `{name}`", path.display()))?;
        dialect::normalize(entry, host)
            .with_context(|| format!("rewriting `{}` made `{name}` unreadable", path.display()))?;
    }
    Ok(())
}

/// Mode for a directory holding credentials: owner only, no group, no other.
const OWNER_ONLY_DIR: u32 = 0o700;

/// Mode for a file holding credentials: owner read/write only.
const OWNER_ONLY_FILE: u32 = 0o600;

/// Restricts a path to its owner.
#[cfg(unix)]
fn restrict(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .with_context(|| format!("cannot restrict permissions on `{}`", path.display()))
}

/// Permission bits are a Unix concept; elsewhere there is nothing to restrict.
#[cfg(not(unix))]
fn restrict(_path: &Path, _mode: u32) -> Result<()> {
    Ok(())
}

/// Writes by renaming a fully written temporary over the target.
///
/// The temporary is narrowed before anything is written into it. These files hold real
/// API keys, and `fs::write` creates at 0644, so a wide-open copy of every credential
/// would otherwise exist for as long as the write takes. The mode of the file being
/// replaced is carried across too: a rename adopts the temporary's permissions, so
/// without this a config the user had deliberately locked down to 0600 would be
/// silently widened by the next deploy.
pub fn write_atomically(path: &Path, text: &str) -> Result<()> {
    let temporary = path.with_extension("mcpctl-tmp");
    std::fs::File::create(&temporary)
        .with_context(|| format!("cannot create `{}`", temporary.display()))?;
    restrict(&temporary, OWNER_ONLY_FILE)?;
    std::fs::write(&temporary, text)
        .with_context(|| format!("cannot write `{}`", temporary.display()))?;

    if let Ok(existing) = std::fs::metadata(path) {
        std::fs::set_permissions(&temporary, existing.permissions()).with_context(|| {
            format!(
                "cannot carry `{}`'s permissions onto its replacement",
                path.display()
            )
        })?;
    }

    std::fs::rename(&temporary, path)
        .with_context(|| format!("cannot replace `{}`", path.display()))?;
    Ok(())
}

/// Copies a file into the backup directory before it is replaced.
///
/// A backup is a byte-for-byte copy of a live config, so it holds every real API key
/// that config holds. `create_dir_all` leaves 0755 behind and `fs::copy` preserves the
/// source's 0644, which together publish those keys to every account on the machine —
/// and unlike the live config, a backup accumulates and is never looked at again. Both
/// the vault and the copy are therefore narrowed to their owner.
pub fn backup(path: &Path, root: &Path) -> Result<()> {
    if let Some(vault) = root.parent() {
        std::fs::create_dir_all(vault)
            .with_context(|| format!("cannot create `{}`", vault.display()))?;
        restrict(vault, OWNER_ONLY_DIR)?;
    }
    std::fs::create_dir_all(root).with_context(|| format!("cannot create `{}`", root.display()))?;
    restrict(root, OWNER_ONLY_DIR)?;

    let flattened = path
        .to_string_lossy()
        .trim_start_matches('/')
        .replace('/', "_");
    let destination = root.join(flattened);
    std::fs::copy(path, &destination)
        .with_context(|| format!("cannot back up `{}`", path.display()))?;
    restrict(&destination, OWNER_ONLY_FILE)?;
    Ok(())
}

/// Whether a process with this name is currently running.
///
/// Reads `/proc` directly rather than shelling out to `pgrep`, which is not present
/// everywhere. Any other platform reports `false`, and the caller falls back to the
/// confirmation prompt.
fn process_running(name: &str) -> bool {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.join("comm").exists() {
            continue;
        }
        if let Ok(comm) = std::fs::read_to_string(path.join("comm"))
            && comm.trim() == name
        {
            return true;
        }
    }
    false
}

/// Filesystem-safe UTC timestamp for the backup directory (Standard §14).
fn timestamp() -> String {
    jiff::Timestamp::now()
        .strftime("%Y%m%dT%H%M%SZ")
        .to_string()
}

/// Backs a file up, then replaces it atomically.
///
/// Shared with [`crate::fill_keys`] so both writers get the same guarantees: a copy
/// under `~/.mcp-backup/<timestamp>/` before anything changes, and a rename over a
/// fully written temporary rather than a truncate-and-write.
pub fn backup_and_write(path: &Path, text: &str) -> Result<()> {
    let backup_root = home_dir()?.join(".mcp-backup").join(timestamp());
    backup(path, &backup_root)?;
    write_atomically(path, text)
}

/// Locates the user's home directory.
pub fn home_dir() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .context("HOME is not set")
}

/// Asks a yes/no question.
///
/// Applying the manifest defaults to yes; deleting something the manifest does not know
/// about defaults to no.
fn confirm(prompt: &str, default_yes: bool) -> Result<bool> {
    print!("{prompt}");
    let _ = std::io::stdout().flush();
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    Ok(match answer.trim().to_ascii_lowercase().as_str() {
        "y" | "yes" => true,
        "" => default_yes,
        _ => false,
    })
}

/// Prints what deploying would do.
fn report(survey: &Survey, dry_run: bool, profile: &Profile) {
    let Survey {
        changes,
        skipped,
        blocked,
    } = survey;
    if profile.json {
        let report = json!({
            "dry_run": dry_run,
            "files": changes.iter().map(|change| json!({
                "host": change.host,
                "path": change.path.display().to_string(),
                "dirty": change.dirty,
                "added": change.added,
                "updated": change.updated,
                "strays": change.strays,
                "strays_prunable": change.prunable,
                "needs_key": change.needs_key,
            })).collect::<Vec<_>>(),
            "skipped": skipped,
            "blocked": blocked,
        });
        profile.emit("mcpctl deploy", &report);
        return;
    }

    let dirty = changes.iter().filter(|change| change.dirty).count();
    println!(
        "{} live file(s) inspected, {dirty} would change",
        changes.len()
    );

    for change in changes {
        if !change.dirty && change.strays.is_empty() {
            continue;
        }
        println!("\n{} — {}", change.host, change.path.display());
        for name in &change.added {
            println!("  + {name}  (added)");
        }
        for name in &change.updated {
            println!("  ~ {name}  (updated)");
        }
        for name in &change.strays {
            let note = if change.prunable {
                "not in the manifest"
            } else {
                "not in the manifest; this host's block also holds its builtins"
            };
            println!("  ! {name}  ({note})");
        }
        if !change.needs_key.is_empty() {
            println!(
                "  ? no value available for {} — omitted rather than writing a \
                 placeholder; run `mcpctl fill-keys`",
                change.needs_key.join(", ")
            );
        }
    }

    if !blocked.is_empty() {
        println!("\nblocked:");
        for entry in blocked {
            println!("  {entry}");
        }
    }
    if !skipped.is_empty() {
        println!("\nskipped:");
        for entry in skipped {
            println!("  {entry}");
        }
    }
}

/// Whether a value still carries an unfilled placeholder token anywhere inside it.
fn contains_placeholder(value: &str, placeholders: &BTreeMap<String, String>) -> bool {
    placeholders.keys().any(|token| value.contains(token))
}

/// Maps each placeholder token to the environment variable that supplies it.
pub fn placeholder_index(manifest: &Manifest) -> BTreeMap<String, String> {
    manifest
        .secrets
        .iter()
        .map(|secret| (secret.placeholder.clone(), secret.env.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Format, content_differs};

    /// The case this function exists for: Claude Code rewrites `~/.claude.json`
    /// in its own layout on exit, and the reserialization used to be reported
    /// as drift forever, because no deploy can out-write the process that owns
    /// the file.
    #[test]
    fn reformatting_alone_is_not_a_difference() {
        let compact = r#"{"mcpServers":{"engram":{"command":"engram","args":["mcp"]}}}"#;
        let pretty = "{\n  \"mcpServers\": {\n    \"engram\": {\n      \"command\": \"engram\",\n      \"args\": [\n        \"mcp\"\n      ]\n    }\n  }\n}\n";

        assert_ne!(compact, pretty, "the fixtures must differ as bytes");
        assert!(
            !content_differs(compact, pretty, Format::Json),
            "layout-only change reported as drift"
        );
    }

    /// Position is preserved for readable diffs, but a reordered map is the
    /// same configuration — rewriting a file to sort it is a wasted write.
    #[test]
    fn key_order_is_not_a_difference() {
        let one = r#"{"a":{"command":"x"},"b":{"command":"y"}}"#;
        let other = r#"{"b":{"command":"y"},"a":{"command":"x"}}"#;

        assert!(!content_differs(one, other, Format::Json));
    }

    #[test]
    fn a_real_value_change_is_still_a_difference() {
        let before = r#"{"mcpServers":{"engram":{"command":"engram"}}}"#;
        let after = r#"{"mcpServers":{"engram":{"command":"engram-next"}}}"#;

        assert!(content_differs(before, after, Format::Json));
    }

    /// Unparseable input must not read as clean: a malformed file still
    /// reports drift rather than being silently skipped.
    #[test]
    fn unparseable_input_falls_back_to_bytes() {
        let broken = "{ this is not json";
        let valid = r#"{"mcpServers":{}}"#;

        assert!(content_differs(broken, valid, Format::Json));
        assert!(
            !content_differs(broken, broken, Format::Json),
            "identical bytes are equal even when unparseable"
        );
    }
}
