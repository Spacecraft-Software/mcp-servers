// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Putting real API keys into the live host configs.
//!
//! This replaces `bin/fill-keys.{sh,nu,ion}`, three parallel shell ports that had to be
//! edited in lockstep and had already drifted apart — none of them knew about
//! `~/.gemini/antigravity-ide/mcp_config.json`.
//!
//! It also works differently, because it has to. The shell scripts substituted the
//! literal token `YOUR_CONTEXT7_API_KEY` wherever it appeared, which required that token
//! to already be sitting in the live file. [`crate::deploy`] deliberately never writes a
//! placeholder into a live config — a rejected credential is worse than an absent one —
//! so there is nothing to substitute. Instead this reads the manifest to learn *which
//! server wants which key under which host-specific field name*, and inserts it.
//!
//! The consequence worth knowing: a key can now be filled into a host whose config never
//! mentioned it, which the string-substituting version could not do.

use std::collections::BTreeMap;
use std::io::IsTerminal;
use std::path::Path;

use anyhow::{Context, Result};
use serde_json::json;

use crate::dialect::{HOSTS, Host, Strictness};
use crate::manifest::Manifest;
use crate::{deploy, dialect, render, splice};

/// Fills every known secret into every live config that wants one.
pub fn run(repo: &Path, yes: bool, as_json: bool) -> Result<()> {
    let manifest = Manifest::load(repo)?;
    let home = deploy::home_dir()?;

    let interactive = !yes
        && std::io::stdin().is_terminal()
        && std::env::var_os("CI").is_none()
        && std::env::var_os("CLAUDECODE").is_none();

    // Environment first, then a hidden prompt. A blank answer skips that key, leaving
    // whatever the live configs already hold untouched.
    let mut values: BTreeMap<String, String> = BTreeMap::new();
    for secret in &manifest.secrets {
        let from_env = std::env::var(&secret.env).ok().filter(|v| !v.is_empty());
        let value = match from_env {
            Some(value) => Some(value),
            None if interactive => {
                let entered =
                    rpassword::prompt_password(format!("{} [blank to skip]: ", secret.env))?;
                Some(entered).filter(|v| !v.is_empty())
            }
            None => None,
        };
        if let Some(value) = value {
            values.insert(secret.env.clone(), value);
        }
    }

    if values.is_empty() {
        if as_json {
            println!(
                "{}",
                json!({ "filled": 0, "reason": "no key values supplied" })
            );
        } else {
            println!(
                "No key values supplied. Export {} or run from a terminal.",
                manifest
                    .secrets
                    .iter()
                    .map(|secret| secret.env.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        return Ok(());
    }

    let mut touched: Vec<String> = Vec::new();
    for host in HOSTS {
        let Some(config) = manifest.host(host.name) else {
            continue;
        };
        for path in config.live_paths(&home) {
            if !path.exists() {
                continue;
            }
            if fill_file(&path, host, &manifest, &values)? {
                touched.push(path.display().to_string());
            }
        }
    }

    if as_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "keys": values.keys().collect::<Vec<_>>(),
                "files": touched,
            }))?
        );
    } else {
        println!(
            "\nFilled {} key(s) into {} file(s).",
            values.len(),
            touched.len()
        );
        for path in &touched {
            println!("  {path}");
        }
        if !touched.is_empty() {
            println!("Restart every host for the new keys to take effect.");
        }
    }
    Ok(())
}

/// Fills secrets into one live config, returning whether anything changed.
fn fill_file(
    path: &Path,
    host: &'static Host,
    manifest: &Manifest,
    values: &BTreeMap<String, String>,
) -> Result<bool> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read `{}`", path.display()))?;
    let root = dialect::parse(host.format, &text, Strictness::Lenient)
        .with_context(|| format!("cannot parse `{}`", path.display()))?;
    let mut servers = splice::servers_of(&root, host.wrapper)
        .with_context(|| format!("cannot read the server block of `{}`", path.display()))?;

    let placeholders = deploy::placeholder_index(manifest);
    let mut changed = false;
    let mut managed: Vec<String> = Vec::new();

    for resolved in manifest.resolve_for(host.name) {
        managed.push(resolved.name.clone());
        let Some(current) = servers.get(&resolved.name) else {
            // Only servers the host already has are filled. Adding one is `deploy`'s job.
            continue;
        };

        // The entry is rebuilt through the renderer rather than patched in place. An
        // earlier version inserted the key straight into the live object, which appended
        // `env` *after* `disabled`, while `deploy` emits it before — so the two commands
        // reordered each other's output forever, rewriting files and stacking up backups
        // on every run. Building both through the same path makes them agree by
        // construction rather than by careful coincidence.
        let rendered = render::server_entry(&resolved, host)?;
        let (filled, _) =
            deploy::resolve_secrets(&rendered, Some(current), host, &placeholders, values);
        if &filled != current {
            servers.insert(resolved.name.clone(), filled);
            changed = true;
        }
    }

    if !changed {
        return Ok(false);
    }

    let new_text = deploy::rewrite(&text, host, &servers)?;
    deploy::verify(&new_text, host, path, &managed)?;
    deploy::backup_and_write(path, &new_text)?;
    Ok(true)
}
