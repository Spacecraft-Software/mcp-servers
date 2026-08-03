// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Host schema dialects.
//!
//! Every host declares the same logical servers but disagrees about nearly every key
//! name. Rather than writing one parser and one renderer per host, the differences are
//! captured as data in [`HOSTS`] and interpreted by shared code. Adding a host is one
//! table row.
//!
//! The traps documented in `CLAUDE.md` each reduce to a single field here: Qwen's
//! `httpUrl` (rather than `url`), Codex's `http_headers` (rather than `headers`),
//! Copilot CLI's missing `mcpServers` wrapper, opencode's `environment` (rather than
//! `env`), and goose's `cmd`/`envs`/`uri`.
//!
//! This table describes *mechanics*. What is declared, and where each host's live
//! config lives, is content and belongs in `mcp.toml`.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use serde_json::Value;

/// Serialization format of a host's config file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Strict JSON.
    Json,
    /// JSON with comments, and — in the wild — trailing commas.
    Jsonc,
    /// TOML.
    Toml,
    /// YAML.
    Yaml,
}

/// How a host spells out the command line of a stdio server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandShape {
    /// `"command": "npx", "args": ["-y", "pkg"]` — the common case.
    Split,
    /// `"command": ["npx", "-y", "pkg"]` — opencode and its forks.
    Joined,
    /// `cmd: npx`, `args: ["-y", "pkg"]` — goose.
    Cmd,
}

impl CommandShape {
    /// Key holding the executable.
    pub fn command_key(self) -> &'static str {
        match self {
            Self::Split | Self::Joined => "command",
            Self::Cmd => "cmd",
        }
    }
}

/// Indentation used when writing a host's file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Indent {
    /// N spaces per level.
    Spaces(usize),
    /// One tab per level.
    Tab,
}

/// How a host expresses that a server is switched on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnabledStyle {
    /// No such concept; presence means enabled.
    Absent,
    /// A positive boolean under this key.
    Enabled(&'static str),
    /// A negated boolean under this key, as in Antigravity's `disabled`.
    Disabled(&'static str),
}

/// A per-server connect timeout a host expects, and the unit it is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timeout {
    /// Key holding the value.
    pub key: &'static str,
    /// Value, in whatever unit the host uses.
    pub value: u64,
}

/// One host application's schema dialect.
#[derive(Debug, Clone, Copy)]
pub struct Host {
    /// Manifest name, matching the repository directory.
    pub name: &'static str,
    /// Path of the tracked template, relative to the repository root.
    pub template: &'static str,
    /// Serialization format.
    pub format: Format,
    /// Key holding the server map, or `None` when servers sit at the top level.
    pub wrapper: Option<&'static str>,
    /// Key holding a remote server's endpoint.
    pub url_field: &'static str,
    /// Key holding a remote server's request headers.
    pub headers_field: &'static str,
    /// Key holding a stdio server's environment variables.
    pub env_field: &'static str,
    /// Spelling of a stdio server's command line.
    pub command_shape: CommandShape,
    /// Indentation of the generated file.
    pub indent: Indent,
    /// Value of the `type` key for a stdio server, when the host wants one.
    pub stdio_type: Option<&'static str>,
    /// Value of the `type` key for a remote server, when the host wants one.
    pub http_type: Option<&'static str>,
    /// How the host switches a server on or off.
    pub enabled_style: EnabledStyle,
    /// Per-server connect timeout the host expects.
    pub timeout: Option<Timeout>,
    /// Whether the host repeats the server's name inside its entry, as goose does.
    pub emit_name_field: bool,
}

/// Every host this repository targets.
pub const HOSTS: &[Host] = &[
    Host {
        name: "Antigravity",
        template: "Antigravity/mcp_config.json",
        format: Format::Json,
        wrapper: Some("mcpServers"),
        url_field: "serverUrl",
        headers_field: "headers",
        env_field: "env",
        command_shape: CommandShape::Split,
        indent: Indent::Spaces(2),
        stdio_type: None,
        http_type: None,
        enabled_style: EnabledStyle::Disabled("disabled"),
        timeout: None,
        emit_name_field: false,
    },
    Host {
        name: "VSCode",
        template: "VSCode/mcp.json",
        format: Format::Json,
        wrapper: Some("servers"),
        url_field: "url",
        headers_field: "headers",
        env_field: "env",
        command_shape: CommandShape::Split,
        indent: Indent::Tab,
        stdio_type: Some("stdio"),
        http_type: Some("http"),
        enabled_style: EnabledStyle::Absent,
        timeout: None,
        emit_name_field: false,
    },
    Host {
        name: "GitHubCopilotCLI",
        template: "GitHubCopilotCLI/mcp-config.json",
        format: Format::Json,
        wrapper: None,
        url_field: "url",
        headers_field: "headers",
        env_field: "env",
        command_shape: CommandShape::Split,
        indent: Indent::Spaces(2),
        stdio_type: Some("stdio"),
        http_type: Some("http"),
        enabled_style: EnabledStyle::Absent,
        timeout: None,
        emit_name_field: false,
    },
    Host {
        name: "ClaudeCode",
        template: "ClaudeCode/.mcp.json",
        format: Format::Json,
        wrapper: Some("mcpServers"),
        url_field: "url",
        headers_field: "headers",
        env_field: "env",
        command_shape: CommandShape::Split,
        indent: Indent::Spaces(2),
        stdio_type: Some("stdio"),
        http_type: Some("http"),
        enabled_style: EnabledStyle::Absent,
        timeout: None,
        emit_name_field: false,
    },
    Host {
        name: "OpenClaude",
        template: "OpenClaude/.mcp.json",
        format: Format::Json,
        wrapper: Some("mcpServers"),
        url_field: "url",
        headers_field: "headers",
        env_field: "env",
        command_shape: CommandShape::Split,
        indent: Indent::Spaces(2),
        stdio_type: Some("stdio"),
        http_type: Some("http"),
        enabled_style: EnabledStyle::Absent,
        timeout: None,
        emit_name_field: false,
    },
    Host {
        name: "Codex",
        template: "Codex/config.toml",
        format: Format::Toml,
        wrapper: Some("mcp_servers"),
        url_field: "url",
        headers_field: "http_headers",
        env_field: "env",
        command_shape: CommandShape::Split,
        indent: Indent::Spaces(2),
        stdio_type: None,
        http_type: None,
        enabled_style: EnabledStyle::Absent,
        timeout: None,
        emit_name_field: false,
    },
    Host {
        name: "Grok",
        template: "Grok/config.toml",
        format: Format::Toml,
        wrapper: Some("mcp_servers"),
        url_field: "url",
        headers_field: "headers",
        env_field: "env",
        command_shape: CommandShape::Split,
        indent: Indent::Spaces(2),
        stdio_type: None,
        http_type: None,
        enabled_style: EnabledStyle::Enabled("enabled"),
        timeout: None,
        emit_name_field: false,
    },
    Host {
        name: "Kimi",
        template: "Kimi/mcp.json",
        format: Format::Json,
        wrapper: Some("mcpServers"),
        url_field: "url",
        headers_field: "headers",
        env_field: "env",
        command_shape: CommandShape::Split,
        indent: Indent::Spaces(2),
        stdio_type: None,
        http_type: None,
        enabled_style: EnabledStyle::Absent,
        timeout: None,
        emit_name_field: false,
    },
    Host {
        name: "Gemini",
        template: "Gemini/settings.json",
        format: Format::Json,
        wrapper: Some("mcpServers"),
        url_field: "url",
        headers_field: "headers",
        env_field: "env",
        command_shape: CommandShape::Split,
        indent: Indent::Spaces(2),
        // Gemini marks remote servers but leaves stdio servers untyped.
        stdio_type: None,
        http_type: Some("http"),
        enabled_style: EnabledStyle::Absent,
        timeout: None,
        emit_name_field: false,
    },
    Host {
        name: "Qwen",
        template: "Qwen/settings.json",
        format: Format::Json,
        wrapper: Some("mcpServers"),
        url_field: "httpUrl",
        headers_field: "headers",
        env_field: "env",
        command_shape: CommandShape::Split,
        indent: Indent::Spaces(2),
        stdio_type: None,
        http_type: None,
        enabled_style: EnabledStyle::Absent,
        timeout: None,
        emit_name_field: false,
    },
    Host {
        name: "OpenCode",
        template: "OpenCode/opencode.jsonc",
        format: Format::Jsonc,
        wrapper: Some("mcp"),
        url_field: "url",
        headers_field: "headers",
        env_field: "environment",
        command_shape: CommandShape::Joined,
        indent: Indent::Spaces(2),
        stdio_type: Some("local"),
        http_type: Some("remote"),
        enabled_style: EnabledStyle::Enabled("enabled"),
        // opencode's default connect timeout is 30s; 60s tolerates a cold `nix run`
        // or `npx` fetch on first launch without the host giving up.
        timeout: Some(Timeout {
            key: "timeout",
            value: 60_000,
        }),
        emit_name_field: false,
    },
    Host {
        name: "Mimo",
        template: "Mimo/mimocode.jsonc",
        format: Format::Jsonc,
        wrapper: Some("mcp"),
        url_field: "url",
        headers_field: "headers",
        env_field: "environment",
        command_shape: CommandShape::Joined,
        indent: Indent::Spaces(2),
        stdio_type: Some("local"),
        http_type: Some("remote"),
        enabled_style: EnabledStyle::Enabled("enabled"),
        timeout: Some(Timeout {
            key: "timeout",
            value: 60_000,
        }),
        emit_name_field: false,
    },
    Host {
        name: "Goose",
        template: "Goose/config.yaml",
        format: Format::Yaml,
        wrapper: Some("extensions"),
        url_field: "uri",
        headers_field: "headers",
        env_field: "envs",
        command_shape: CommandShape::Cmd,
        indent: Indent::Spaces(2),
        stdio_type: Some("stdio"),
        http_type: Some("streamable_http"),
        enabled_style: EnabledStyle::Enabled("enabled"),
        // goose counts in seconds, not milliseconds.
        timeout: Some(Timeout {
            key: "timeout",
            value: 300,
        }),
        emit_name_field: true,
    },
];

/// Looks up a host's dialect by name.
pub fn host_by_name(name: &str) -> Option<&'static Host> {
    HOSTS.iter().find(|host| host.name == name)
}

/// A server's invocation, stripped of host-specific spelling.
///
/// Two hosts declare the same server correctly when their entries normalize to equal
/// values, which is what makes a dialect-agnostic parity check possible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transport {
    /// A locally spawned process speaking MCP over stdio.
    Stdio {
        /// Executable name.
        command: String,
        /// Arguments, in order.
        args: Vec<String>,
        /// Environment overrides. Ordered so comparison is spelling-independent.
        env: BTreeMap<String, String>,
    },
    /// A remote endpoint spoken to over HTTP.
    Http {
        /// Endpoint URL.
        url: String,
        /// Request headers.
        headers: BTreeMap<String, String>,
    },
}

impl Transport {
    /// Short human-readable rendering, for drift reports.
    pub fn summary(&self) -> String {
        match self {
            Self::Stdio { command, args, env } => {
                let mut out = command.clone();
                for arg in args {
                    out.push(' ');
                    out.push_str(arg);
                }
                if !env.is_empty() {
                    let vars: Vec<&str> = env.keys().map(String::as_str).collect();
                    out.push_str("  [env: ");
                    out.push_str(&vars.join(", "));
                    out.push(']');
                }
                out
            }
            Self::Http { url, headers } => {
                if headers.is_empty() {
                    url.clone()
                } else {
                    let names: Vec<&str> = headers.keys().map(String::as_str).collect();
                    format!("{url}  [headers: {}]", names.join(", "))
                }
            }
        }
    }
}

/// A host's declared servers, in file order.
#[derive(Debug, Clone)]
pub struct Servers {
    /// Server name and normalized invocation, in the order the file declares them.
    pub entries: Vec<(String, Transport)>,
}

impl Servers {
    /// Looks up one server by name.
    pub fn get(&self, name: &str) -> Option<&Transport> {
        self.entries
            .iter()
            .find(|(known, _)| known == name)
            .map(|(_, transport)| transport)
    }

    /// Every declared server name, in file order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|(name, _)| name.as_str())
    }
}

/// How forgiving a parse should be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strictness {
    /// Hold `.json` to the JSON grammar. Correct for tracked templates, which must be
    /// copyable verbatim into a host that may only accept strict JSON.
    Strict,
    /// Accept comments and trailing commas even in a `.json` file. Correct for live
    /// configs: the real VS Code `mcp.json` on this machine carries trailing commas.
    Lenient,
}

/// Parses any supported config format into a common JSON value.
pub fn parse(format: Format, text: &str, strictness: Strictness) -> Result<Value> {
    match format {
        Format::Json if strictness == Strictness::Strict => {
            serde_json::from_str(text).context("invalid JSON")
        }
        Format::Json | Format::Jsonc => jsonc_parser::parse_to_serde_value::<Value>(
            text,
            &jsonc_parser::ParseOptions::default(),
        )
        .context("invalid JSON"),
        Format::Toml => toml::from_str(text).context("invalid TOML"),
        Format::Yaml => yaml_serde::from_str(text).context("invalid YAML"),
    }
}

/// Reads a host's config file and normalizes every server it declares.
pub fn load(path: &Path, host: &Host, strictness: Strictness) -> Result<Servers> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read `{}`", path.display()))?;
    let root = parse(host.format, &text, strictness)
        .with_context(|| format!("cannot parse `{}`", path.display()))?;
    servers_from_value(&root, host)
        .with_context(|| format!("cannot interpret `{}`", path.display()))
}

/// Extracts and normalizes the server map out of an already-parsed document.
pub fn servers_from_value(root: &Value, host: &Host) -> Result<Servers> {
    let map = match host.wrapper {
        Some(key) => root
            .get(key)
            .ok_or_else(|| anyhow!("missing `{key}` block"))?,
        None => root,
    };
    let map = map
        .as_object()
        .ok_or_else(|| anyhow!("server block is not an object"))?;

    let mut entries = Vec::with_capacity(map.len());
    for (name, raw) in map {
        // Not every key inside a wrapper is a server: opencode carries `$schema` and
        // Gemini carries `_comment` at the top level, and Copilot has no wrapper at
        // all, so its whole document is the server map.
        if name.starts_with('$') || name.starts_with('_') {
            continue;
        }
        let transport = normalize(raw, host).with_context(|| format!("server `{name}`"))?;
        entries.push((name.clone(), transport));
    }
    Ok(Servers { entries })
}

/// Converts one host-dialect server entry into its dialect-agnostic form.
pub fn normalize(raw: &Value, host: &Host) -> Result<Transport> {
    let obj = raw
        .as_object()
        .ok_or_else(|| anyhow!("entry is not an object"))?;

    if let Some(url) = obj.get(host.url_field).and_then(Value::as_str) {
        let headers = string_map(obj.get(host.headers_field))
            .context("headers must be a string-to-string map")?;
        return Ok(Transport::Http {
            url: url.to_owned(),
            headers,
        });
    }

    let env = string_map(obj.get(host.env_field))
        .context("environment must be a string-to-string map")?;

    let (command, args) = match host.command_shape {
        CommandShape::Split | CommandShape::Cmd => {
            let key = host.command_shape.command_key();
            let command = obj
                .get(key)
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("neither `{}` nor `{key}` present", host.url_field))?;
            (command.to_owned(), string_list(obj.get("args"))?)
        }
        CommandShape::Joined => {
            let list = string_list(obj.get("command"))?;
            let mut parts = list.into_iter();
            let command = parts
                .next()
                .ok_or_else(|| anyhow!("`command` array is empty"))?;
            (command, parts.collect())
        }
    };

    Ok(Transport::Stdio { command, args, env })
}

/// Reads an optional array of strings, defaulting to empty.
fn string_list(value: Option<&Value>) -> Result<Vec<String>> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let array = value
        .as_array()
        .ok_or_else(|| anyhow!("expected an array of strings"))?;
    array
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_owned)
                .ok_or_else(|| anyhow!("array contains a non-string element"))
        })
        .collect()
}

/// Reads an optional string-to-string map, defaulting to empty.
///
/// Numbers and booleans are accepted and stringified: TOML and YAML round-trip
/// `NO_COLOR = "1"` inconsistently across hosts, and treating that as a type error
/// would report drift that does not exist.
fn string_map(value: Option<&Value>) -> Result<BTreeMap<String, String>> {
    let Some(value) = value else {
        return Ok(BTreeMap::new());
    };
    let object = value
        .as_object()
        .ok_or_else(|| anyhow!("expected an object"))?;
    let mut out = BTreeMap::new();
    for (key, item) in object {
        let rendered = match item {
            Value::String(text) => text.clone(),
            Value::Number(number) => number.to_string(),
            Value::Bool(flag) => flag.to_string(),
            other => bail!("value for `{key}` is not a scalar: {other}"),
        };
        out.insert(key.clone(), rendered);
    }
    Ok(out)
}
