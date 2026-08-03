// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Generation of the tracked host templates from `mcp.toml`.
//!
//! Each host's document is assembled as an ordered [`Value`] and then handed to the
//! matching serializer in [`crate::emit`]. Field order is canonical rather than
//! per-host: order carries no meaning in JSON, TOML, or YAML, and imposing one order
//! everywhere keeps the renderer small.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};

use crate::dialect::{CommandShape, EnabledStyle, Format, HOSTS, Host};
use crate::emit;
use crate::manifest::{Manifest, OrderedMap, Resolved, TransportKind};

/// One generated file and its intended location.
#[derive(Debug)]
pub struct Output {
    /// Path relative to the repository root.
    pub path: PathBuf,
    /// Full file contents.
    pub text: String,
}

/// Generates every template, writing them unless `check_only`.
pub fn run(repo: &Path, check_only: bool, as_json: bool) -> Result<()> {
    let manifest = Manifest::load(repo)?;
    let outputs = generate(&manifest)?;

    let mut differing: Vec<String> = Vec::new();
    let mut written: Vec<String> = Vec::new();

    for output in &outputs {
        let target = repo.join(&output.path);
        let current = std::fs::read_to_string(&target).unwrap_or_default();
        let display = output.path.display().to_string();
        if current == output.text {
            continue;
        }
        if check_only {
            differing.push(display);
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("cannot create `{}`", parent.display()))?;
            }
            std::fs::write(&target, &output.text)
                .with_context(|| format!("cannot write `{}`", target.display()))?;
            written.push(display);
        }
    }

    if as_json {
        let report = json!({
            "ok": differing.is_empty(),
            "generated": outputs.len(),
            "differing": differing,
            "written": written,
        });
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else if check_only {
        if differing.is_empty() {
            println!(
                "{} files generated, all match the tracked templates",
                outputs.len()
            );
        } else {
            println!(
                "{} of {} files differ from the manifest:",
                differing.len(),
                outputs.len()
            );
            for path in &differing {
                println!("  {path}");
            }
            println!("\nrun `mcpctl render` to regenerate them");
        }
    } else if written.is_empty() {
        println!("{} files generated, nothing changed", outputs.len());
    } else {
        println!("{} of {} files updated:", written.len(), outputs.len());
        for path in &written {
            println!("  {path}");
        }
    }

    if check_only && !differing.is_empty() {
        std::process::exit(1);
    }
    Ok(())
}

/// Builds every output file from the manifest.
pub fn generate(manifest: &Manifest) -> Result<Vec<Output>> {
    let mut outputs = Vec::new();
    for host in HOSTS {
        let config = manifest
            .host(host.name)
            .with_context(|| format!("manifest has no entry for host `{}`", host.name))?;
        let servers = manifest.resolve_for(host.name);

        outputs.push(Output {
            path: PathBuf::from(host.template),
            text: render_host(host, &servers, manifest, config.header.as_deref(), config)?,
        });

        if let Some(path) = &config.companion_disabled_file {
            let disabled: Vec<&str> = servers
                .iter()
                .filter(|server| !server.enabled)
                .map(|server| server.name.as_str())
                .collect();
            let mut root = Map::new();
            root.insert("disabledMcpjsonServers".to_owned(), json!(disabled));
            outputs.push(Output {
                path: PathBuf::from(path),
                text: emit::json(&Value::Object(root), host.indent),
            });
        }

        if let Some(path) = &config.companion_enablement_file {
            let mut root = Map::new();
            for server in &servers {
                root.insert(server.name.clone(), json!({ "enabled": server.enabled }));
            }
            outputs.push(Output {
                path: PathBuf::from(path),
                text: emit::json(&Value::Object(root), host.indent),
            });
        }
    }
    Ok(outputs)
}

/// Renders one host's main template.
fn render_host(
    host: &Host,
    servers: &[Resolved],
    manifest: &Manifest,
    header: Option<&str>,
    config: &crate::manifest::HostConfig,
) -> Result<String> {
    let mut entries = Map::new();
    for server in servers {
        entries.insert(server.name.clone(), server_entry(server, host)?);
    }

    match host.format {
        Format::Toml => {
            let wrapper = host
                .wrapper
                .context("a TOML host must nest its servers under a table")?;
            Ok(emit::toml(wrapper, &entries, header))
        }
        Format::Yaml => {
            let wrapper = host
                .wrapper
                .context("a YAML host must nest its servers under a key")?;
            Ok(emit::yaml(wrapper, &entries, header))
        }
        Format::Json | Format::Jsonc => {
            let mut root = Map::new();
            if let Some(url) = &config.schema_url {
                root.insert("$schema".to_owned(), json!(url));
            }
            if let Some(wrapper) = host.wrapper {
                // JSON cannot hold comments, so a host that wants explanatory text gets
                // it as a `_comment` key. JSONC hosts could take a real comment, but
                // none of them currently ask for one.
                if let Some(text) = header {
                    root.insert("_comment".to_owned(), json!(text.trim()));
                }
                root.insert(wrapper.to_owned(), Value::Object(entries));
            } else {
                if header.is_some() {
                    bail!(
                        "host `{}` has no wrapper key, so it cannot carry a header",
                        host.name
                    );
                }
                root = entries;
            }
            if config.emit_inputs {
                let inputs: Vec<Value> = manifest
                    .secrets
                    .iter()
                    .map(|secret| {
                        json!({
                            "id": secret.env,
                            "type": "promptString",
                            "description": secret.description,
                            "password": true,
                        })
                    })
                    .collect();
                root.insert("inputs".to_owned(), json!(inputs));
            }
            Ok(emit::json(&Value::Object(root), host.indent))
        }
    }
}

/// Renders one server in one host's dialect.
pub fn server_entry(server: &Resolved, host: &Host) -> Result<Value> {
    let mut entry = Map::new();

    if host.emit_name_field {
        entry.insert("name".to_owned(), json!(server.name));
    }

    let declared_type = match server.transport {
        TransportKind::Stdio => host.stdio_type,
        TransportKind::Http => host.http_type,
    };
    if let Some(kind) = declared_type {
        entry.insert("type".to_owned(), json!(kind));
    }

    match server.transport {
        TransportKind::Stdio => {
            let command = server
                .command
                .as_deref()
                .with_context(|| format!("server `{}` has no command", server.name))?;
            match host.command_shape {
                CommandShape::Split | CommandShape::Cmd => {
                    entry.insert(host.command_shape.command_key().to_owned(), json!(command));
                    if !server.args.is_empty() {
                        entry.insert("args".to_owned(), json!(server.args));
                    }
                }
                CommandShape::Joined => {
                    let mut parts = vec![command.to_owned()];
                    parts.extend(server.args.iter().cloned());
                    entry.insert("command".to_owned(), json!(parts));
                }
            }
            if !server.env.is_empty() {
                entry.insert(host.env_field.to_owned(), ordered(&server.env));
            }
        }
        TransportKind::Http => {
            let url = server
                .url
                .as_deref()
                .with_context(|| format!("server `{}` has no url", server.name))?;
            entry.insert(host.url_field.to_owned(), json!(url));
            if !server.headers.is_empty() {
                entry.insert(host.headers_field.to_owned(), ordered(&server.headers));
            }
        }
    }

    match host.enabled_style {
        EnabledStyle::Absent => {}
        EnabledStyle::Enabled(key) => {
            entry.insert(key.to_owned(), json!(server.enabled));
        }
        EnabledStyle::Disabled(key) => {
            entry.insert(key.to_owned(), json!(!server.enabled));
        }
    }

    if let Some(timeout) = host.timeout {
        entry.insert(timeout.key.to_owned(), json!(timeout.value));
    }

    Ok(Value::Object(entry))
}

/// Converts an ordered manifest map into a JSON object, preserving order.
fn ordered(map: &OrderedMap) -> Value {
    let mut out = Map::new();
    for (key, value) in map {
        out.insert(key.clone(), json!(value));
    }
    Value::Object(out)
}
