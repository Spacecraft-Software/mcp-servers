// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `mcp.toml` manifest: what is declared, and where each host keeps its files.
//!
//! The manifest owns content. Dialect mechanics — which key holds a URL, whether
//! environment variables go under `env` or `environment` — are facts about a host's
//! schema and live in [`crate::dialect::HOSTS`] instead. Keeping them apart means a
//! server change never requires touching Rust, and a newly supported host never
//! requires touching the manifest's server list.

use std::collections::{BTreeMap, BTreeSet};
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
    /// Whether this host does not carry the server at all.
    ///
    /// Distinct from `enabled = false`, which still emits the entry and only switches
    /// it off. Claude Code hides a claude.ai connector whose URL a locally configured
    /// server already claims, so for those the entry has to be absent, not off.
    #[serde(default)]
    pub omit: bool,
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
        Self::parse(&text).with_context(|| format!("cannot parse `{}`", path.display()))
    }

    /// Parses and validates manifest text.
    ///
    /// Split out of [`Self::load`] so the validation rules can be exercised without a
    /// repository on disk.
    pub fn parse(text: &str) -> Result<Self> {
        let manifest: Self = toml::from_str(text)?;
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
            for (host, entry) in &server.overrides {
                if !HOSTS.iter().any(|known| known.name == host) {
                    bail!(
                        "server `{}` declares an override for unknown host `{host}`",
                        server.name
                    );
                }
                // An omitted server is never emitted, so any field alongside `omit`
                // would silently do nothing. Say so rather than accept it.
                if entry.omit && entry.replaces_a_field() {
                    bail!(
                        "server `{}` is omitted on host `{host}` but also overrides \
                         fields there; an omitted server is never emitted, so those \
                         have no effect",
                        server.name
                    );
                }
            }
        }
        Ok(())
    }

    /// Applies `host`'s overrides to every server it carries, in manifest order.
    ///
    /// Servers the host omits are filtered out here rather than at each call site.
    /// Every caller — `render`, `deploy`, `fill-keys` — means "the servers this host
    /// has", and asking each to remember the filter is how a server ends up shipped
    /// somewhere it was declared absent.
    pub fn resolve_for(&self, host: &str) -> Vec<Resolved> {
        self.servers
            .iter()
            .filter(|server| !server.is_omitted_for(host))
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

    /// Names `host` deliberately does not carry.
    ///
    /// `check` compares templates against each other and needs this to tell a declared
    /// omission from a template someone edited by hand.
    pub fn omitted_for(&self, host: &str) -> BTreeSet<&str> {
        self.servers
            .iter()
            .filter(|server| server.is_omitted_for(host))
            .map(|server| server.name.as_str())
            .collect()
    }

    /// Looks up a host's manifest entry.
    pub fn host(&self, name: &str) -> Option<&HostConfig> {
        self.hosts.iter().find(|host| host.name == name)
    }
}

impl Server {
    /// Whether `host` deliberately does not carry this server at all.
    pub fn is_omitted_for(&self, host: &str) -> bool {
        self.overrides.get(host).is_some_and(|entry| entry.omit)
    }
}

impl Override {
    /// Whether this override replaces any field, as opposed to only setting `omit`.
    fn replaces_a_field(&self) -> bool {
        self.command.is_some()
            || self.args.is_some()
            || self.env.is_some()
            || self.url.is_some()
            || self.headers.is_some()
            || self.enabled.is_some()
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
