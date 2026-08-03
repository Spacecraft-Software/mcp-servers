// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Serializers for the three config formats.
//!
//! Everything is built as an ordered [`serde_json::Value`] first — `serde_json` is
//! compiled with `preserve_order`, so key insertion order survives — and then written
//! out by one of these. Hand-rolled rather than delegating to each format's own
//! serializer, because the output has to stay close to files a human wrote: arrays of
//! strings inline rather than exploded one element per line, and no reordering.

use std::fmt::Write as _;

use serde_json::{Map, Value};

use crate::dialect::Indent;

impl Indent {
    /// One level of indentation as text.
    fn unit(self) -> &'static str {
        match self {
            Self::Spaces(2) => "  ",
            Self::Spaces(_) => "    ",
            Self::Tab => "\t",
        }
    }

    /// `depth` levels of indentation.
    fn at(self, depth: usize) -> String {
        self.unit().repeat(depth)
    }
}

/// Serializes a value as JSON, with a trailing newline.
///
/// Arrays whose elements are all scalars are written on one line; this is what the
/// hand-written templates do, and exploding them would turn every regeneration into a
/// large and unreviewable diff.
pub fn json(value: &Value, indent: Indent) -> String {
    let mut out = String::new();
    write_json(&mut out, value, indent, 0);
    out.push('\n');
    out
}

/// Serializes a value as JSON already nested `depth` levels deep, with no trailing
/// newline.
///
/// Used when splicing a block back into a live config: the opening brace follows
/// `"mcpServers": ` on an existing line, so only the interior and the closing brace
/// need indenting, and they must line up with the file around them.
pub fn json_fragment(value: &Value, indent: Indent, depth: usize) -> String {
    let mut out = String::new();
    write_json(&mut out, value, indent, depth);
    out
}

/// Writes one JSON value at `depth`.
fn write_json(out: &mut String, value: &Value, indent: Indent, depth: usize) {
    match value {
        Value::Object(map) if map.is_empty() => out.push_str("{}"),
        Value::Object(map) => {
            out.push_str("{\n");
            let last = map.len() - 1;
            for (index, (key, item)) in map.iter().enumerate() {
                out.push_str(&indent.at(depth + 1));
                out.push_str(&escape_json(key));
                out.push_str(": ");
                write_json(out, item, indent, depth + 1);
                if index != last {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str(&indent.at(depth));
            out.push('}');
        }
        Value::Array(items) if items.is_empty() => out.push_str("[]"),
        Value::Array(items) if items.iter().all(is_scalar) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index != 0 {
                    out.push_str(", ");
                }
                write_json(out, item, indent, depth);
            }
            out.push(']');
        }
        Value::Array(items) => {
            out.push_str("[\n");
            let last = items.len() - 1;
            for (index, item) in items.iter().enumerate() {
                out.push_str(&indent.at(depth + 1));
                write_json(out, item, indent, depth + 1);
                if index != last {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str(&indent.at(depth));
            out.push(']');
        }
        Value::String(text) => out.push_str(&escape_json(text)),
        Value::Number(number) => out.push_str(&number.to_string()),
        Value::Bool(flag) => out.push_str(if *flag { "true" } else { "false" }),
        Value::Null => out.push_str("null"),
    }
}

/// Whether a value fits on one line inside an inline array.
fn is_scalar(value: &Value) -> bool {
    matches!(
        value,
        Value::String(_) | Value::Number(_) | Value::Bool(_) | Value::Null
    )
}

/// Quotes and escapes a JSON string.
fn escape_json(text: &str) -> String {
    // `serde_json` already implements exactly the escaping rules; reuse them rather
    // than hand-rolling a second, subtly different version.
    Value::String(text.to_owned()).to_string()
}

/// Serializes a server map as TOML tables under `wrapper`.
///
/// Scalar keys are emitted first and nested objects afterwards as their own tables,
/// because a TOML table header ends the key/value block that precedes it.
pub fn toml(wrapper: &str, servers: &Map<String, Value>, header: Option<&str>) -> String {
    let mut out = String::new();
    if let Some(text) = header {
        for line in text.trim().lines() {
            out.push_str("# ");
            out.push_str(line);
            out.push('\n');
        }
        out.push('\n');
    }

    for (name, entry) in servers {
        let Some(fields) = entry.as_object() else {
            continue;
        };
        let _ = writeln!(out, "[{wrapper}.{name}]");
        for (key, value) in fields {
            if value.is_object() {
                continue;
            }
            let _ = writeln!(out, "{key} = {}", toml_scalar(value));
        }
        out.push('\n');

        for (key, value) in fields {
            let Some(nested) = value.as_object() else {
                continue;
            };
            // Emitted even when empty: an empty table is meaningful to whoever wrote it,
            // and these serializers rewrite host-owned entries as well as ours.
            let _ = writeln!(out, "[{wrapper}.{name}.{key}]");
            for (inner_key, inner) in nested {
                let _ = writeln!(out, "{inner_key} = {}", toml_scalar(inner));
            }
            out.push('\n');
        }
    }
    out
}

/// Renders one TOML scalar or array of scalars.
fn toml_scalar(value: &Value) -> String {
    match value {
        Value::Array(items) => {
            let rendered: Vec<String> = items.iter().map(toml_scalar).collect();
            format!("[{}]", rendered.join(", "))
        }
        Value::String(text) => escape_json(text),
        Value::Number(number) => number.to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Null => "\"\"".to_owned(),
        Value::Object(_) => String::new(),
    }
}

/// Serializes a server map as YAML under `wrapper`.
pub fn yaml(wrapper: &str, servers: &Map<String, Value>, header: Option<&str>) -> String {
    let mut out = String::new();
    if let Some(text) = header {
        for line in text.trim().lines() {
            out.push_str("# ");
            out.push_str(line);
            out.push('\n');
        }
    }
    let _ = writeln!(out, "{wrapper}:");
    for (name, entry) in servers {
        let _ = writeln!(out, "  {name}:");
        let Some(fields) = entry.as_object() else {
            continue;
        };
        for (key, value) in fields {
            match value {
                // An empty collection is written as `[]` / `{}` rather than skipped.
                // These serializers also rewrite entries belonging to the host itself —
                // goose keeps `available_tools: []` on its builtin extensions — and
                // dropping a key there is silent data loss in someone else's config.
                Value::Array(items) if items.is_empty() => {
                    let _ = writeln!(out, "    {key}: []");
                }
                Value::Array(items) => {
                    let _ = writeln!(out, "    {key}:");
                    for item in items {
                        let _ = writeln!(out, "      - {}", yaml_scalar(item));
                    }
                }
                Value::Object(nested) if nested.is_empty() => {
                    let _ = writeln!(out, "    {key}: {{}}");
                }
                Value::Object(nested) => {
                    let _ = writeln!(out, "    {key}:");
                    for (inner_key, inner) in nested {
                        let _ = writeln!(out, "      {inner_key}: {}", yaml_scalar(inner));
                    }
                }
                scalar => {
                    let _ = writeln!(out, "    {key}: {}", yaml_scalar(scalar));
                }
            }
        }
    }
    out
}

/// Renders one YAML scalar, quoting only where a plain scalar would change meaning.
fn yaml_scalar(value: &Value) -> String {
    match value {
        Value::String(text) => {
            if yaml_needs_quotes(text) {
                escape_json(text)
            } else {
                text.clone()
            }
        }
        Value::Number(number) => number.to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Null => "null".to_owned(),
        other => other.to_string(),
    }
}

/// Whether a plain YAML scalar would be misread and therefore needs quoting.
///
/// Deliberately narrow. Over-quoting is safe for YAML but produces a large diff
/// against the hand-written file, so only the cases that actually change meaning are
/// quoted: values that would parse as another type (`1` as a number, `true` as a
/// boolean) and values whose first character is a YAML indicator.
fn yaml_needs_quotes(text: &str) -> bool {
    if text.is_empty() {
        return true;
    }
    if text.trim() != text {
        return true;
    }
    if text.parse::<f64>().is_ok() {
        return true;
    }
    if matches!(
        text.to_ascii_lowercase().as_str(),
        "true" | "false" | "null" | "yes" | "no" | "on" | "off" | "~"
    ) {
        return true;
    }
    // A leading indicator character changes how the scalar is parsed. `-` is only an
    // indicator when followed by a space, so `--` and `-y` stay unquoted.
    let first = text.as_bytes()[0];
    if b"@`#&*!|>'\"%{}[],?:".contains(&first) {
        return true;
    }
    if text.starts_with("- ") {
        return true;
    }
    // `key: value` inside a plain scalar would start a nested mapping; ` #` starts a
    // comment. A bare colon (as in a URL) is harmless.
    text.contains(": ") || text.contains(" #")
}
