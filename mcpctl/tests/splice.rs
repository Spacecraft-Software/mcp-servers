// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for editing a live config in place.
//!
//! Several of these pin bugs found by deploying into a sandboxed copy of the real
//! configs rather than by reading the code, and each would be silent in production:
//! a dropped `available_tools: []` looks like nothing happened until goose stops
//! offering a builtin's tools.

use mcpctl::dialect::Indent;
use mcpctl::emit;
use mcpctl::splice;
use serde_json::json;

#[test]
fn splicing_leaves_everything_outside_the_block_untouched() {
    let original = r#"{
  "numFailedLogins": 3,
  "mcpServers": {
    "old": { "command": "x" }
  },
  "projects": { "keep": "me" }
}
"#;
    let block = splice::locate_json(original, Some("mcpServers")).expect("block is found");
    let replacement = emit::json_fragment(
        &json!({ "new": { "command": "y" } }),
        Indent::Spaces(2),
        block.depth,
    );
    let result = splice::apply(original, block, &replacement);

    assert!(result.contains("\"numFailedLogins\": 3"));
    assert!(result.contains("\"projects\": { \"keep\": \"me\" }"));
    assert!(result.contains("\"new\""));
    assert!(!result.contains("\"old\""));
    serde_json::from_str::<serde_json::Value>(&result).expect("result is still valid JSON");
}

#[test]
fn spliced_block_is_indented_to_match_its_surroundings() {
    let original = "{\n  \"mcpServers\": {\n    \"a\": { \"command\": \"x\" }\n  }\n}\n";
    let block = splice::locate_json(original, Some("mcpServers")).expect("block is found");
    assert_eq!(block.depth, 1, "the wrapper key sits one level in");

    let replacement = emit::json_fragment(
        &json!({ "a": { "command": "x" } }),
        Indent::Spaces(2),
        block.depth,
    );
    let result = splice::apply(original, block, &replacement);
    assert!(
        result.contains("\n    \"a\": {"),
        "server entries land two levels in, not at the margin:\n{result}"
    );
    assert!(
        result.contains("\n  }\n}"),
        "closing brace lines up with the key"
    );
}

#[test]
fn a_tab_indented_file_stays_tab_indented() {
    let original = "{\n\t\"servers\": {\n\t\t\"a\": { \"command\": \"x\" }\n\t}\n}\n";
    assert_eq!(splice::detect_indent(original), Indent::Tab);
    let block = splice::locate_json(original, Some("servers")).expect("block is found");
    assert_eq!(block.depth, 1);
}

#[test]
fn yaml_block_ends_at_the_next_top_level_key() {
    let original =
        "telemetry: true\nextensions:\n  a:\n    type: stdio\n    cmd: x\nprovider: openai\n";
    let block = splice::locate_yaml(original, "extensions").expect("block is found");
    let result = splice::apply(original, block, "extensions:\n  b:\n    cmd: y\n");

    assert!(result.starts_with("telemetry: true\n"));
    assert!(result.trim_end().ends_with("provider: openai"));
    assert!(!result.contains("type: stdio"));
}

/// An empty collection belonging to another tool must survive a rewrite.
///
/// goose keeps `available_tools: []` on its builtin extensions. An earlier version of
/// the YAML writer skipped empty values, which silently stripped that key from 13
/// extensions that this repository does not own.
#[test]
fn empty_collections_are_preserved_not_dropped() {
    let mut servers = serde_json::Map::new();
    servers.insert(
        "builtin".to_owned(),
        json!({
            "type": "platform",
            "bundled": true,
            "available_tools": [],
            "settings": {},
        }),
    );

    let yaml = emit::yaml("extensions", &servers, None);
    assert!(yaml.contains("available_tools: []"), "got:\n{yaml}");
    assert!(yaml.contains("settings: {}"), "got:\n{yaml}");

    let parsed: serde_json::Value = yaml_serde::from_str(&yaml).expect("valid YAML");
    let entry = &parsed["extensions"]["builtin"];
    assert_eq!(entry["available_tools"], json!([]));
    assert_eq!(entry["settings"], json!({}));
}

#[test]
fn yaml_quotes_only_what_would_change_meaning() {
    let mut servers = serde_json::Map::new();
    servers.insert(
        "s".to_owned(),
        json!({
            "cmd": "npx",
            "envs": { "NO_COLOR": "1", "RUST_LOG": "error", "PKG": "@scope/name" },
            "uri": "https://example.com/mcp",
        }),
    );
    let yaml = emit::yaml("extensions", &servers, None);

    assert!(
        yaml.contains("NO_COLOR: \"1\""),
        "a numeric string stays a string"
    );
    assert!(
        yaml.contains("RUST_LOG: error"),
        "a plain word needs no quotes"
    );
    assert!(
        yaml.contains("PKG: \"@scope/name\""),
        "a leading @ is an indicator"
    );
    assert!(
        yaml.contains("uri: https://example.com/mcp"),
        "a bare colon in a URL is harmless"
    );

    let parsed: serde_json::Value = yaml_serde::from_str(&yaml).expect("valid YAML");
    assert_eq!(parsed["extensions"]["s"]["envs"]["NO_COLOR"], json!("1"));
}

#[test]
fn toml_rewrite_keeps_unrelated_sections_verbatim() {
    let original = "model = \"gpt-5\"\n\n[history]\npersistence = \"save-all\"\n\n[mcp_servers.old]\ncommand = \"x\"\n";
    let mut servers = serde_json::Map::new();
    servers.insert(
        "new".to_owned(),
        json!({ "command": "y", "args": ["a"], "env": { "K": "v" } }),
    );

    let result =
        splice::replace_toml_block(original, "mcp_servers", &servers).expect("rewrite succeeds");

    assert!(result.contains("model = \"gpt-5\""));
    assert!(result.contains("[history]"));
    assert!(result.contains("persistence = \"save-all\""));
    assert!(result.contains("[mcp_servers.new]"));
    assert!(!result.contains("[mcp_servers.old]"));

    let parsed: toml::Value = toml::from_str(&result).expect("valid TOML");
    assert_eq!(parsed["mcp_servers"]["new"]["env"]["K"].as_str(), Some("v"));
}

#[test]
fn nested_tool_tables_round_trip_through_both_toml_writers() {
    // The two TOML writers have to agree byte-for-byte: `render` produces the
    // tracked template, `deploy` splices the same servers into a live file, and
    // a disagreement between them reads as permanent drift.
    let servers = json!({
        "engram": {
            "command": "engram",
            "tools": { "save_chat": { "approval_mode": "approve" } }
        }
    })
    .as_object()
    .expect("object")
    .clone();

    let whole_file = emit::toml("mcp_servers", &servers, None);
    let spliced = splice::replace_toml_block("", "mcp_servers", &servers).expect("splices");

    for (label, text) in [("emit", &whole_file), ("splice", &spliced)] {
        let parsed: toml::Value = toml::from_str(text).unwrap_or_else(|e| panic!("{label}: {e}"));
        assert_eq!(
            parsed["mcp_servers"]["engram"]["tools"]["save_chat"]["approval_mode"].as_str(),
            Some("approve"),
            "{label} lost the per-tool setting"
        );
        // A bare `[mcp_servers.engram.tools]` is a namespace TOML creates
        // implicitly; writing it is a stray line the host never had.
        assert!(
            !text.contains("[mcp_servers.engram.tools]\n"),
            "{label} wrote a redundant namespace header:\n{text}"
        );
    }
}

#[test]
fn a_genuinely_empty_table_keeps_its_header() {
    // Distinct from the namespace case above: an empty table carries intent
    // (goose's `available_tools = []` equivalents), so it must survive.
    let servers = json!({ "goose": { "cmd": "x", "extra": {} } })
        .as_object()
        .expect("object")
        .clone();

    let text = emit::toml("mcp_servers", &servers, None);
    assert!(
        text.contains("[mcp_servers.goose.extra]"),
        "empty table lost its header:\n{text}"
    );
}
