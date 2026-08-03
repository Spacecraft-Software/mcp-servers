// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `mcp.toml` manifest: what is declared, and where each host keeps its files.
//!
//! The manifest owns content. Dialect mechanics — which key holds a URL, whether
//! environment variables go under `env` or `environment` — are facts about a host's
//! schema and live in [`crate::dialect::HOSTS`] instead. Keeping them apart means a
//! server change never requires touching Rust, and a newly supported host never
//! requires touching the manifest's server list.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use indexmap::IndexMap;
use serde::Deserialize;

use crate::dialect::HOSTS;

/// Ordered string map, so `env` and `headers` keep the order they were written in.
pub type OrderedMap = IndexMap<String, String>;

/// The parsed manifest.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Placeholder tokens substituted at fill time, in the order VS Code prompts.
    #[serde(default)]
    pub secrets: Vec<Secret>,
    /// Every declared server, in the order hosts should list them.
    pub servers: Vec<Server>,
    /// Per-host file locations and flags.
    pub hosts: Vec<HostConfig>,
}

/// One API key placeholder.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Secret {
    /// Environment variable carrying the real value.
    pub env: String,
    /// Token appearing in templates in place of the real value.
    pub placeholder: String,
    /// Human-readable label, used for VS Code's prompt.
    pub description: String,
}

/// How a server is reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransportKind {
    /// A locally spawned process speaking MCP over stdio.
    Stdio,
    /// A remote HTTP endpoint.
    Http,
}

/// A declared server.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Server {
    /// Server identifier, used as the key in every host config.
    pub name: String,
    /// How the server is reached.
    pub transport: TransportKind,
    /// Executable, for stdio servers.
    #[serde(default)]
    pub command: Option<String>,
    /// Arguments, for stdio servers.
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment overrides, for stdio servers.
    #[serde(default)]
    pub env: OrderedMap,
    /// Endpoint, for HTTP servers.
    #[serde(default)]
    pub url: Option<String>,
    /// Request headers, for HTTP servers.
    #[serde(default)]
    pub headers: OrderedMap,
    /// Whether hosts should ship the server switched on.
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
    /// Differences that are correct for a specific host, keyed by host name.
    #[serde(default)]
    pub overrides: BTreeMap<String, Override>,
}

/// Default for [`Server::enabled`].
fn enabled_by_default() -> bool {
    true
}

/// A per-host difference that is deliberate rather than drift.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Override {
    /// Replacement executable.
    pub command: Option<String>,
    /// Replacement arguments.
    pub args: Option<Vec<String>>,
    /// Replacement environment.
    pub env: Option<OrderedMap>,
    /// Replacement endpoint.
    pub url: Option<String>,
    /// Replacement headers.
    pub headers: Option<OrderedMap>,
    /// Replacement enabled state.
    pub enabled: Option<bool>,
}

/// Per-host file locations and behavioral flags.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostConfig {
    /// Host name, matching an entry in [`HOSTS`].
    pub name: String,
    /// Live config paths under `$HOME`, with `~` unexpanded.
    pub live: Vec<String>,
    /// Explanatory text placed at the top of the generated template.
    #[serde(default)]
    pub header: Option<String>,
    /// Value of a `$schema` key, emitted first when present.
    #[serde(default)]
    pub schema_url: Option<String>,
    /// Whether to generate VS Code's `inputs` array from [`Manifest::secrets`].
    #[serde(default)]
    pub emit_inputs: bool,
    /// Template path of a companion file listing disabled servers.
    #[serde(default)]
    pub companion_disabled_file: Option<String>,
    /// Live path of the companion file listing disabled servers.
    #[serde(default)]
    pub companion_disabled_live: Option<String>,
    /// Template path of a companion file enabling servers.
    #[serde(default)]
    pub companion_enablement_file: Option<String>,
    /// Live path of the companion file enabling servers.
    #[serde(default)]
    pub companion_enablement_live: Option<String>,
    /// Whether `deploy` may offer to remove servers it does not manage.
    ///
    /// False for goose, whose `extensions` block also holds its builtin extensions.
    #[serde(default = "enabled_by_default")]
    pub prune_strays: bool,
    /// Process name that owns and rewrites this config while it runs.
    ///
    /// Claude Code rewrites `~/.claude.json` on exit, so a deploy applied underneath a
    /// running instance is silently reverted. `deploy` refuses rather than racing it.
    #[serde(default)]
    pub guard_process: Option<String>,
}

/// A server with its host-specific overrides already applied.
#[derive(Debug, Clone)]
pub struct Resolved {
    /// Server identifier.
    pub name: String,
    /// How the server is reached.
    pub transport: TransportKind,
    /// Executable, for stdio servers.
    pub command: Option<String>,
    /// Arguments, for stdio servers.
    pub args: Vec<String>,
    /// Environment overrides, for stdio servers.
    pub env: OrderedMap,
    /// Endpoint, for HTTP servers.
    pub url: Option<String>,
    /// Request headers, for HTTP servers.
    pub headers: OrderedMap,
    /// Whether the host should ship the server switched on.
    pub enabled: bool,
}

impl Manifest {
    /// Reads and validates the manifest at the repository root.
    pub fn load(repo: &Path) -> Result<Self> {
        let path = repo.join("mcp.toml");
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("cannot read `{}`", path.display()))?;
        let manifest: Self =
            toml::from_str(&text).with_context(|| format!("cannot parse `{}`", path.display()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Rejects a manifest that cannot produce a complete, consistent set of templates.
    fn validate(&self) -> Result<()> {
        for host in &self.hosts {
            if !HOSTS.iter().any(|known| known.name == host.name) {
                bail!(
                    "manifest declares host `{}`, which has no dialect in \
                     `mcpctl/src/dialect.rs`",
                    host.name
                );
            }
        }
        for known in HOSTS {
            if !self.hosts.iter().any(|host| host.name == known.name) {
                bail!(
                    "dialect defines host `{}`, but the manifest does not declare it",
                    known.name
                );
            }
        }

        for server in &self.servers {
            match server.transport {
                TransportKind::Stdio if server.command.is_none() => {
                    bail!("server `{}` is stdio but has no `command`", server.name);
                }
                TransportKind::Http if server.url.is_none() => {
                    bail!("server `{}` is http but has no `url`", server.name);
                }
                _ => {}
            }
            for host in server.overrides.keys() {
                if !HOSTS.iter().any(|known| known.name == host) {
                    bail!(
                        "server `{}` declares an override for unknown host `{host}`",
                        server.name
                    );
                }
            }
        }
        Ok(())
    }

    /// Applies `host`'s overrides to every server, in manifest order.
    pub fn resolve_for(&self, host: &str) -> Vec<Resolved> {
        self.servers
            .iter()
            .map(|server| {
                let overrides = server.overrides.get(host);
                Resolved {
                    name: server.name.clone(),
                    transport: server.transport,
                    command: overrides
                        .and_then(|entry| entry.command.clone())
                        .or_else(|| server.command.clone()),
                    args: overrides
                        .and_then(|entry| entry.args.clone())
                        .unwrap_or_else(|| server.args.clone()),
                    env: overrides
                        .and_then(|entry| entry.env.clone())
                        .unwrap_or_else(|| server.env.clone()),
                    url: overrides
                        .and_then(|entry| entry.url.clone())
                        .or_else(|| server.url.clone()),
                    headers: overrides
                        .and_then(|entry| entry.headers.clone())
                        .unwrap_or_else(|| server.headers.clone()),
                    enabled: overrides
                        .and_then(|entry| entry.enabled)
                        .unwrap_or(server.enabled),
                }
            })
            .collect()
    }

    /// Looks up a host's manifest entry.
    pub fn host(&self, name: &str) -> Option<&HostConfig> {
        self.hosts.iter().find(|host| host.name == name)
    }
}

impl HostConfig {
    /// Live config paths with `~` expanded against `home`.
    pub fn live_paths(&self, home: &Path) -> Vec<PathBuf> {
        self.live
            .iter()
            .map(|entry| expand_home(entry, home))
            .collect()
    }
}

/// Expands a leading `~/` against `home`.
pub fn expand_home(path: &str, home: &Path) -> PathBuf {
    match path.strip_prefix("~/") {
        Some(rest) => home.join(rest),
        None => PathBuf::from(path),
    }
}
