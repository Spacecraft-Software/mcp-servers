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
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::{Map, Value, json};

use crate::dialect::{Format, HOSTS, Host, Strictness};
use crate::manifest::{HostConfig, Manifest};
use crate::{dialect, emit, render, splice};

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
#[expect(
    clippy::struct_excessive_bools,
    reason = "these mirror independent command-line flags, so they are not a state enum"
)]
pub struct Options {
    /// Report what would change without writing anything.
    pub dry_run: bool,
    /// Skip every confirmation prompt.
    pub yes: bool,
    /// Deploy even while a host that owns its config is running.
    pub force: bool,
    /// Emit machine-readable JSON.
    pub as_json: bool,
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
pub fn run(repo: &Path, options: Options, host_filter: Option<&str>) -> Result<()> {
    let manifest = Manifest::load(repo)?;
    let home = home_dir()?;

    // Writing is opt-in. Without a terminal to confirm at, and without an explicit
    // --yes, the only safe reading of the request is "show me what you would do" — this
    // is what keeps an agent-driven or CI run from rewriting a developer's configs.
    let interactive = !options.yes
        && std::io::stdin().is_terminal()
        && std::env::var_os("CI").is_none()
        && std::env::var_os("CLAUDECODE").is_none();
    let dry_run = options.dry_run || (!options.yes && !interactive);

    let survey = survey(&manifest, &home, host_filter, options.force)?;
    report(&survey, dry_run, options.as_json)?;

    if dry_run {
        if !options.as_json {
            println!("\nnothing written (dry run). Re-run with --yes from a terminal to apply.");
        }
        return Ok(());
    }

    let written = apply(&survey.changes, &home, interactive)?;
    if !options.as_json {
        println!("\n{written} file(s) written");
        println!("Restart every host — a failed MCP server is not retried in-process.");
    }
    Ok(())
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

    Ok(Change {
        host: host.name,
        dialect: Some(host),
        path: path.to_path_buf(),
        dirty: new_text != text,
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

    let updated = if new_text == text {
        Vec::new()
    } else {
        vec![path.file_name().map_or_else(
            || "companion".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        )]
    };

    Ok(Change {
        host: host.name,
        dialect: None,
        path: path.to_path_buf(),
        dirty: new_text != text,
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

/// Writes by renaming a fully written temporary over the target.
fn write_atomically(path: &Path, text: &str) -> Result<()> {
    let temporary = path.with_extension("mcpctl-tmp");
    std::fs::write(&temporary, text)
        .with_context(|| format!("cannot write `{}`", temporary.display()))?;
    std::fs::rename(&temporary, path)
        .with_context(|| format!("cannot replace `{}`", path.display()))?;
    Ok(())
}

/// Copies a file into the backup directory before it is replaced.
fn backup(path: &Path, root: &Path) -> Result<()> {
    std::fs::create_dir_all(root).with_context(|| format!("cannot create `{}`", root.display()))?;
    let flattened = path
        .to_string_lossy()
        .trim_start_matches('/')
        .replace('/', "_");
    std::fs::copy(path, root.join(flattened))
        .with_context(|| format!("cannot back up `{}`", path.display()))?;
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
fn report(survey: &Survey, dry_run: bool, as_json: bool) -> Result<()> {
    let Survey {
        changes,
        skipped,
        blocked,
    } = survey;
    if as_json {
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
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
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
    Ok(())
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
