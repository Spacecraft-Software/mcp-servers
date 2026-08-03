// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Replacing just the MCP block inside a live host config.
//!
//! Live configs are not ours. `~/.claude.json` is 238 KB across 6,800 lines, of which
//! the MCP block is a few dozen — the rest is project history that Claude Code owns.
//! Re-serializing the whole document would rewrite all of it, so the MCP block is
//! located by byte range and spliced, leaving every other byte untouched.
//!
//! TOML is the exception: `toml_edit` already preserves the formatting of everything it
//! does not modify, so there is no reason to do range surgery by hand.

use anyhow::{Context, Result, anyhow, bail};
use jsonc_parser::ast::{ObjectPropName, Value as AstValue};
use jsonc_parser::{CollectOptions, ParseOptions, parse_to_ast};
use serde_json::{Map, Value};

use crate::dialect::Indent;

/// Where a block sits in a file, and how deeply it is indented.
#[derive(Debug, Clone, Copy)]
pub struct Block {
    /// Byte offset of the first character of the block.
    pub start: usize,
    /// Byte offset just past the last character of the block.
    pub end: usize,
    /// Indentation depth of the key introducing the block.
    pub depth: usize,
}

/// Finds the JSON value held under `wrapper`, or the whole document when `None`.
pub fn locate_json(text: &str, wrapper: Option<&str>) -> Result<Block> {
    let parsed = parse_to_ast(text, &CollectOptions::default(), &ParseOptions::default())
        .context("invalid JSON")?;
    let root = parsed
        .value
        .ok_or_else(|| anyhow!("file contains no JSON value"))?;

    let Some(wrapper) = wrapper else {
        let range = value_range(&root);
        return Ok(Block {
            start: range.0,
            end: range.1,
            depth: 0,
        });
    };

    let AstValue::Object(object) = &root else {
        bail!("document root is not an object")
    };
    let property = object
        .properties
        .iter()
        .find(|property| match &property.name {
            ObjectPropName::String(name) => name.value == wrapper,
            ObjectPropName::Word(name) => name.value == wrapper,
        })
        .ok_or_else(|| anyhow!("no `{wrapper}` block"))?;

    let (start, end) = value_range(&property.value);
    Ok(Block {
        start,
        end,
        depth: depth_of(text, property.range.start),
    })
}

/// Byte range of an AST value.
fn value_range(value: &AstValue<'_>) -> (usize, usize) {
    let range = match value {
        AstValue::StringLit(node) => node.range,
        AstValue::NumberLit(node) => node.range,
        AstValue::BooleanLit(node) => node.range,
        AstValue::Object(node) => node.range,
        AstValue::Array(node) => node.range,
        AstValue::NullKeyword(node) => node.range,
    };
    (range.start, range.end)
}

/// Indentation depth of the line containing `offset`, counted in indent units.
///
/// Both space- and tab-indented files appear here, and the unit is whatever the file
/// already uses, so the count is derived from the prefix rather than assumed.
fn depth_of(text: &str, offset: usize) -> usize {
    let line_start = text[..offset].rfind('\n').map_or(0, |index| index + 1);
    let prefix: &str = &text[line_start..offset];
    if prefix.starts_with('\t') {
        prefix
            .chars()
            .filter(|character| *character == '\t')
            .count()
    } else {
        let spaces = prefix.chars().filter(|character| *character == ' ').count();
        // Two-space indentation is near-universal in these files; four is the only
        // other width seen, and halving it would misplace the closing brace.
        if spaces % 2 == 0 { spaces / 2 } else { 0 }
    }
}

/// Finds the `wrapper:` block in a YAML document, including its introducing line.
///
/// The block runs to the next line that starts at column zero, which is where the next
/// top-level key begins.
pub fn locate_yaml(text: &str, wrapper: &str) -> Result<Block> {
    let needle = format!("{wrapper}:");
    let mut offset = 0usize;
    let mut start = None;

    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\n', '\r']);
        match start {
            None => {
                if trimmed == needle || trimmed.starts_with(&format!("{needle} ")) {
                    start = Some(offset);
                }
            }
            Some(begin) => {
                // A line beginning at column zero is the next top-level key, which ends
                // the block. Blank and comment lines belong to whatever follows.
                if !trimmed.is_empty()
                    && !trimmed.starts_with(char::is_whitespace)
                    && !trimmed.starts_with('#')
                {
                    return Ok(Block {
                        start: begin,
                        end: offset,
                        depth: 0,
                    });
                }
            }
        }
        offset += line.len();
    }

    start.map_or_else(
        || Err(anyhow!("no `{wrapper}:` block")),
        |begin| {
            Ok(Block {
                start: begin,
                end: text.len(),
                depth: 0,
            })
        },
    )
}

/// Replaces `block` in `text` with `replacement`.
pub fn apply(text: &str, block: Block, replacement: &str) -> String {
    let mut out = String::with_capacity(text.len() + replacement.len());
    out.push_str(&text[..block.start]);
    out.push_str(replacement);
    out.push_str(&text[block.end..]);
    out
}

/// Detects the indentation a JSON-ish file already uses.
///
/// Writing two-space indentation into a tab-indented file would work but would show up
/// as a whole-file diff to whoever owns that config.
pub fn detect_indent(text: &str) -> Indent {
    for line in text.lines() {
        if line.starts_with('\t') {
            return Indent::Tab;
        }
        let spaces = line.len() - line.trim_start_matches(' ').len();
        if spaces > 0 {
            return Indent::Spaces(spaces);
        }
    }
    Indent::Spaces(2)
}

/// Replaces the `wrapper` table of a TOML document, leaving everything else as written.
///
/// `toml_edit` already preserves the spans it does not touch, so unlike the JSON and
/// YAML paths this needs no byte arithmetic.
pub fn replace_toml_block(
    text: &str,
    wrapper: &str,
    servers: &Map<String, Value>,
) -> Result<String> {
    let mut document: toml_edit::DocumentMut = text.parse().context("invalid TOML")?;
    let mut table = toml_edit::Table::new();
    // Implicit, so the output is a run of `[mcp_servers.<name>]` headers with no bare
    // `[mcp_servers]` line above them — matching how these files are already written.
    table.set_implicit(true);
    for (name, entry) in servers {
        table.insert(name, toml_item(entry));
    }
    document.insert(wrapper, toml_edit::Item::Table(table));
    Ok(document.to_string())
}

/// Converts a JSON value into a TOML item.
fn toml_item(value: &Value) -> toml_edit::Item {
    match value {
        Value::Object(map) => {
            let mut table = toml_edit::Table::new();
            // A TOML table header ends the key/value block before it, so every scalar
            // has to be written before the first sub-table.
            for (key, item) in map.iter().filter(|(_, item)| !item.is_object()) {
                table.insert(key, toml_item(item));
            }
            for (key, item) in map.iter().filter(|(_, item)| item.is_object()) {
                table.insert(key, toml_item(item));
            }
            toml_edit::Item::Table(table)
        }
        other => toml_edit::Item::Value(toml_value(other)),
    }
}

/// Converts a JSON scalar or array into a TOML value.
fn toml_value(value: &Value) -> toml_edit::Value {
    match value {
        Value::String(text) => text.as_str().into(),
        Value::Bool(flag) => (*flag).into(),
        Value::Number(number) => number.as_i64().map_or_else(
            || number.as_f64().unwrap_or_default().into(),
            toml_edit::Value::from,
        ),
        Value::Array(items) => {
            let mut array = toml_edit::Array::new();
            for item in items {
                array.push(toml_value(item));
            }
            array.into()
        }
        // TOML has no null, and nothing in a server entry is ever null.
        Value::Null | Value::Object(_) => "".into(),
    }
}

/// Reads the server map out of a parsed live document.
///
/// Returns the map itself so unmanaged keys survive: a live config may hold servers
/// this repository does not declare, and they are not ours to drop.
pub fn servers_of(root: &Value, wrapper: Option<&str>) -> Result<Map<String, Value>> {
    let block = match wrapper {
        Some(key) => root.get(key).ok_or_else(|| anyhow!("no `{key}` block"))?,
        None => root,
    };
    block
        .as_object()
        .cloned()
        .ok_or_else(|| anyhow!("the server block is not an object"))
}
