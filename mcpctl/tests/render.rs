// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Round-trip tests: everything the manifest declares must survive being written out
//! in a host's dialect and read back.
//!
//! These run against the real `mcp.toml` and the real dialect table rather than
//! fixtures, so a dialect entry that is internally consistent but wrong for its host
//! still shows up here as soon as the manifest gains a server that exercises it.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;

use mcpctl::dialect::{self, HOSTS, Strictness, Transport};
use mcpctl::manifest::{Manifest, TransportKind};
use mcpctl::render;

/// Repository root, one level above the crate.
fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate always has a parent directory")
        .to_path_buf()
}

/// Loads the manifest, failing the test with its own error message.
fn manifest() -> Manifest {
    Manifest::load(&repo()).expect("mcp.toml must load")
}

#[test]
fn every_generated_template_parses() {
    let manifest = manifest();
    let outputs = render::generate(&manifest).expect("generation must succeed");
    for output in &outputs {
        let name = output.path.to_string_lossy().to_string();
        let Some(host) = HOSTS.iter().find(|host| host.template == name) else {
            // Companion files (enablement, disabled lists) have no dialect entry.
            continue;
        };
        dialect::parse(host.format, &output.text, Strictness::Strict)
            .unwrap_or_else(|error| panic!("generated `{name}` does not parse: {error:#}"));
    }
}

#[test]
fn every_host_declares_the_servers_it_resolves() {
    let manifest = manifest();
    let outputs = render::generate(&manifest).expect("generation must succeed");

    for host in HOSTS {
        // Per host, not once for all of them: a host may declare `omit = true` and not
        // carry a server at all, and `resolve_for` is the only thing that knows.
        let resolved = manifest.resolve_for(host.name);
        let expected: Vec<&str> = resolved.iter().map(|server| server.name.as_str()).collect();

        let output = outputs
            .iter()
            .find(|output| output.path.to_string_lossy() == host.template)
            .unwrap_or_else(|| panic!("no output generated for `{}`", host.name));
        let value = dialect::parse(host.format, &output.text, Strictness::Strict)
            .expect("generated output parses");
        let servers = dialect::servers_from_value(&value, host)
            .unwrap_or_else(|error| panic!("`{}` is not interpretable: {error:#}", host.name));

        let declared: Vec<&str> = servers.names().collect();
        assert_eq!(
            declared, expected,
            "`{}` does not declare its resolved servers in order",
            host.name
        );
    }
}

/// Servers claude.ai ships a connector for, paired with the host in their URL.
///
/// Claude Code hides a claude.ai connector whose URL a locally configured server
/// already claims. `enabled = false` does not clear it: a disabled server is still
/// written into `~/.claude.json`, and it is presence of the URL that does the hiding.
/// So these are omitted from Claude Code outright — and only from Claude Code, because
/// no other host has a connector to fall back on.
const CLAUDE_AI_SUPPLIES: &[(&str, &str)] = &[
    ("context7", "mcp.context7.com"),
    ("microsoft-learn", "learn.microsoft.com"),
];

#[test]
fn claude_code_omits_what_claude_ai_supplies() {
    let manifest = manifest();
    let outputs = render::generate(&manifest).expect("generation must succeed");

    let claude = outputs
        .iter()
        .find(|output| output.path.to_string_lossy() == "ClaudeCode/.mcp.json")
        .expect("the Claude Code template is generated");

    for (name, url_host) in CLAUDE_AI_SUPPLIES {
        assert!(
            !claude.text.contains(&format!("\"{name}\"")),
            "ClaudeCode/.mcp.json still declares `{name}`"
        );
        // Assert the URL too, not just the key: the hiding is keyed on the endpoint, so
        // renaming the server would not be enough to bring the connector back.
        assert!(
            !claude.text.contains(url_host),
            "ClaudeCode/.mcp.json still claims {url_host}, which hides the connector"
        );
    }

    for host in HOSTS.iter().filter(|host| host.name != "ClaudeCode") {
        let declared: Vec<String> = manifest
            .resolve_for(host.name)
            .into_iter()
            .map(|server| server.name)
            .collect();
        for (name, _) in CLAUDE_AI_SUPPLIES {
            assert!(
                declared.iter().any(|entry| entry == name),
                "`{}` must still carry `{name}` — the omission is Claude Code only",
                host.name
            );
        }
    }
}

#[test]
fn omission_does_not_leak_into_claude_codes_disabled_list() {
    let outputs = render::generate(&manifest()).expect("generation must succeed");

    let settings = outputs
        .iter()
        .find(|output| output.path.to_string_lossy() == "ClaudeCode/settings.json")
        .expect("the Claude Code companion is generated");
    let value: serde_json::Value =
        serde_json::from_str(&settings.text).expect("the companion is JSON");

    assert_eq!(
        value["disabledMcpjsonServers"],
        serde_json::json!(["sequential-thinking"]),
        "an omitted server must not turn into a disabled one — a disabled server is \
         still written into ~/.claude.json, which is exactly what hides the connector"
    );
}

#[test]
fn an_omitted_server_may_not_also_override_fields() {
    // `omit` wins over every other key, so a manifest carrying both is a mistake worth
    // naming rather than a silent no-op.
    //
    // Every host has to be declared or `validate` bails on the host cross-check first
    // and the test would pass without ever reaching the rule it is about.
    let hosts = HOSTS.iter().fold(String::new(), |mut text, host| {
        let _ = writeln!(
            text,
            "\n[[hosts]]\nname = \"{}\"\nlive = [\"~/x\"]",
            host.name
        );
        text
    });
    let text = format!(
        r#"
[[servers]]
name = "context7"
transport = "http"
url = "https://mcp.context7.com/mcp"

[servers.overrides.ClaudeCode]
omit = true
url = "https://example.invalid/mcp"
{hosts}"#
    );

    let error = Manifest::parse(&text)
        .expect_err("a manifest that both omits and overrides must be rejected");
    let message = format!("{error:#}");
    assert!(
        message.contains("omitted on host `ClaudeCode`"),
        "rejected for the wrong reason: {message}"
    );
}

#[test]
fn round_trip_preserves_every_invocation() {
    let manifest = manifest();
    let outputs = render::generate(&manifest).expect("generation must succeed");

    for host in HOSTS {
        let output = outputs
            .iter()
            .find(|output| output.path.to_string_lossy() == host.template)
            .expect("output exists for every host");
        let value = dialect::parse(host.format, &output.text, Strictness::Strict)
            .expect("generated output parses");
        let servers =
            dialect::servers_from_value(&value, host).expect("generated output is interpretable");

        for resolved in manifest.resolve_for(host.name) {
            let actual = servers
                .get(&resolved.name)
                .unwrap_or_else(|| panic!("`{}` lost server `{}`", host.name, resolved.name));

            let expected = match resolved.transport {
                TransportKind::Stdio => Transport::Stdio {
                    command: resolved
                        .command
                        .clone()
                        .expect("stdio server has a command"),
                    args: resolved.args.clone(),
                    env: resolved
                        .env
                        .iter()
                        .map(|(key, value)| (key.clone(), value.clone()))
                        .collect::<BTreeMap<_, _>>(),
                },
                TransportKind::Http => Transport::Http {
                    url: resolved.url.clone().expect("http server has a url"),
                    headers: resolved
                        .headers
                        .iter()
                        .map(|(key, value)| (key.clone(), value.clone()))
                        .collect::<BTreeMap<_, _>>(),
                },
            };

            assert_eq!(
                *actual, expected,
                "`{}` changed the invocation of `{}` on the way through its dialect",
                host.name, resolved.name
            );
        }
    }
}

/// Pins the host quirks that `CLAUDE.md` documents as traps.
///
/// The round-trip test above reads and writes through the same dialect entry, so a
/// field name that is wrong in the same way on both sides survives it. These
/// assertions check the generated text against what each host actually expects, and
/// are the guard against someone "tidying" the table into consistency.
#[test]
fn documented_host_quirks_survive() {
    let manifest = manifest();
    let outputs = render::generate(&manifest).expect("generation must succeed");
    let text = |template: &str| -> String {
        outputs
            .iter()
            .find(|output| output.path.to_string_lossy() == template)
            .unwrap_or_else(|| panic!("no output for `{template}`"))
            .text
            .clone()
    };

    let qwen = text("Qwen/settings.json");
    assert!(
        qwen.contains("\"httpUrl\""),
        "Qwen keys remote servers with `httpUrl`"
    );
    assert!(
        !qwen.contains("\"url\""),
        "Qwen has no `url` key; using one silently disables the server"
    );

    let codex = text("Codex/config.toml");
    assert!(
        codex.contains("[mcp_servers.context7.http_headers]"),
        "Codex spells remote headers `http_headers`"
    );

    let copilot = text("GitHubCopilotCLI/mcp-config.json");
    assert!(
        !copilot.contains("\"mcpServers\""),
        "Copilot CLI keys servers at the top level, with no wrapper"
    );

    let opencode = text("OpenCode/opencode.jsonc");
    assert!(
        opencode.contains("\"environment\""),
        "opencode spells stdio env vars `environment`"
    );
    assert!(
        opencode.contains("\"command\": [\"npx\""),
        "opencode takes one joined command array, not command plus args"
    );

    let goose = text("Goose/config.yaml");
    assert!(
        goose.contains("cmd: npx"),
        "goose spells the executable `cmd`"
    );
    assert!(
        goose.contains("envs:"),
        "goose spells stdio env vars `envs`"
    );
    assert!(
        goose.contains("uri: https://mcp.context7.com/mcp"),
        "goose spells a remote endpoint `uri`"
    );
    assert!(
        goose.contains("NO_COLOR: \"1\""),
        "a numeric-looking env value must stay a quoted string in YAML"
    );

    let vscode = text("VSCode/mcp.json");
    assert!(
        vscode.contains("\"${input:CONTEXT7_API_KEY}\"".trim_matches('"'))
            && vscode.contains("\"inputs\""),
        "VS Code resolves secrets through a generated `inputs` array"
    );
    assert!(
        !vscode.contains("YOUR_CONTEXT7_API_KEY"),
        "a literal placeholder must never reach the VS Code template"
    );
    assert!(vscode.contains('\t'), "VS Code's file is tab-indented");
}

/// Servers whose upstream writes its log lines to **stdout**, where the host is reading
/// JSON-RPC, paired with the environment that silences them.
///
/// `crates-mcp` and `mcp-server-terminal` both emit `tracing` output on stdout rather
/// than stderr, coloured by default, so the first bytes a host reads are an ANSI escape
/// and not a message. Hosts that skip unparseable lines (Claude Code, opencode) tolerate
/// it; Antigravity's Go client does not, and answers `calling "initialize": invalid
/// character '\x1b' looking for beginning of value`.
///
/// `RUST_LOG=error` is the entry that actually fixes it — it drops the WARN/INFO pair
/// that would otherwise precede the handshake. `NO_COLOR` and `TERM` strip the escape
/// sequences from anything that still reaches the stream, so an error-level line
/// degrades to a skippable text line rather than a parse failure.
const STDOUT_LOGGERS: &[(&str, &[(&str, &str)])] = &[
    (
        "crates",
        &[("RUST_LOG", "error"), ("NO_COLOR", "1"), ("TERM", "dumb")],
    ),
    (
        "terminal",
        &[("RUST_LOG", "error"), ("NO_COLOR", "1"), ("TERM", "dumb")],
    ),
];

/// Pins the environment that keeps a stdout-logging server's stream parseable.
///
/// `check` compares hosts against each other, so an `env` dropped from a manifest entry
/// is dropped from all thirteen at once and still looks perfectly consistent — which is
/// how `crates` shipped without `RUST_LOG` and broke only on the one host strict enough
/// to notice. This asserts against the resolved value per host, so a per-host override
/// cannot quietly remove it either.
#[test]
fn servers_that_log_to_stdout_stay_silenced() {
    let manifest = manifest();
    for host in HOSTS {
        for resolved in manifest.resolve_for(host.name) {
            let Some((_, required)) = STDOUT_LOGGERS
                .iter()
                .find(|(name, _)| *name == resolved.name)
            else {
                continue;
            };
            for (key, value) in *required {
                assert_eq!(
                    resolved.env.get(*key).map(String::as_str),
                    Some(*value),
                    "`{}` on `{}` must set {key}={value}; without it the server's own log \
                     output lands on stdout and corrupts the JSON-RPC stream",
                    resolved.name,
                    host.name
                );
            }
        }
    }
}

#[test]
fn generation_is_deterministic() {
    let manifest = manifest();
    let first = render::generate(&manifest).expect("generation must succeed");
    let second = render::generate(&manifest).expect("generation must succeed");
    for (left, right) in first.iter().zip(second.iter()) {
        assert_eq!(left.path, right.path);
        assert_eq!(
            left.text,
            right.text,
            "`{}` is not deterministic",
            left.path.display()
        );
    }
}
