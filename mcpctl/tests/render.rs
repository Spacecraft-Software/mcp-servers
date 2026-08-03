// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Round-trip tests: everything the manifest declares must survive being written out
//! in a host's dialect and read back.
//!
//! These run against the real `mcp.toml` and the real dialect table rather than
//! fixtures, so a dialect entry that is internally consistent but wrong for its host
//! still shows up here as soon as the manifest gains a server that exercises it.

use std::collections::BTreeMap;
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
fn every_host_declares_every_server() {
    let manifest = manifest();
    let outputs = render::generate(&manifest).expect("generation must succeed");
    let expected: Vec<&str> = manifest
        .servers
        .iter()
        .map(|server| server.name.as_str())
        .collect();

    for host in HOSTS {
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
            "`{}` does not declare the manifest's servers in order",
            host.name
        );
    }
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
