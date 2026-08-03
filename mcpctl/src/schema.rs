// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Machine-readable description of the command surface.
//!
//! The output is JSON Schema Draft 2020-12, which is byte-for-byte what `Anthropic`'s
//! `tools[].input_schema` and `MCP`'s `tools[].inputSchema` expect, so an agent can paste
//! it into a function-calling request with no translation. `OpenAI` and `Gemini` need a thin
//! wrapper, which `--format` supplies.
//!
//! Every `description` is filled in: `OpenAI` rejects a parameter without one, and an
//! agent reading the schema to decide which command to call has nothing else to go on.

use serde_json::{Value, json};

/// Wrapper shape to emit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Format {
    /// Raw JSON Schema Draft 2020-12. Identical to what `Anthropic` and `MCP` accept.
    Json,
    /// `Anthropic` `tools[]` entries.
    Anthropic,
    /// `OpenAI` `tools[].function` entries.
    Openai,
    /// `Gemini` `function_declarations[]` entries.
    Gemini,
    /// `MCP` `tools[]` entries.
    Mcp,
}

/// One command's callable surface.
struct Command {
    name: &'static str,
    description: &'static str,
    properties: Value,
    required: &'static [&'static str],
    examples: &'static [&'static str],
}

/// Every command, in the order `--help` lists them.
fn commands() -> Vec<Command> {
    vec![
        Command {
            name: "mcpctl_check",
            description: "Verify every tracked host template parses and that all hosts \
                          declare the same MCP servers with the same invocations. Exits \
                          non-zero on drift.",
            properties: json!({
                "repo": {
                    "type": "string",
                    "description": "Repository root. Defaults to the nearest ancestor containing mcp.toml.",
                },
                "json": {
                    "type": "boolean",
                    "description": "Emit a JSON envelope instead of prose.",
                },
            }),
            required: &[],
            examples: &["mcpctl check --json"],
        },
        Command {
            name: "mcpctl_render",
            description: "Regenerate every host template from the mcp.toml manifest. \
                          Templates are generated artifacts; edit the manifest, not them.",
            properties: json!({
                "check": {
                    "type": "boolean",
                    "description": "Do not write. Exit non-zero if any template differs from the manifest.",
                },
                "repo": {
                    "type": "string",
                    "description": "Repository root. Defaults to the nearest ancestor containing mcp.toml.",
                },
            }),
            required: &[],
            examples: &["mcpctl render --check --json", "mcpctl render"],
        },
        Command {
            name: "mcpctl_deploy",
            description: "Push the manifest into the live host configs under $HOME. \
                          Replaces only the MCP block, preserves servers it does not \
                          manage, backs up before writing, and refuses a host whose \
                          process is running.",
            properties: json!({
                "dry_run": {
                    "type": "boolean",
                    "description": "Report what would change without writing. Implied when no terminal is present or an agent is detected.",
                },
                "yes": {
                    "type": "boolean",
                    "description": "Skip confirmation prompts. Required to write when no terminal is present.",
                },
                "host": {
                    "type": "string",
                    "description": "Restrict to one host by manifest name, such as Codex or Goose.",
                },
                "force": {
                    "type": "boolean",
                    "description": "Deploy to a host even while the process that owns its config is running. The write may be reverted by that process.",
                },
            }),
            required: &[],
            examples: &[
                "mcpctl deploy --dry-run --json",
                "mcpctl deploy --yes",
                "mcpctl deploy --host Codex --dry-run",
            ],
        },
        Command {
            name: "mcpctl_fill_keys",
            description: "Write real API keys into the live host configs, reading values \
                          from the environment or prompting with echo off. A supplied \
                          value overrides what is already there, so this also rotates a key.",
            properties: json!({
                "yes": {
                    "type": "boolean",
                    "description": "Skip prompts and use only values already present in the environment.",
                },
            }),
            required: &[],
            examples: &["CONTEXT7_API_KEY=ctx7sk-... mcpctl fill-keys --yes"],
        },
        Command {
            name: "mcpctl_describe",
            description: "Report how this invocation resolved: output format, color, \
                          interactivity, and which agent is calling.",
            properties: json!({}),
            required: &[],
            examples: &["mcpctl describe --json"],
        },
    ]
}

/// Builds the schema document in the requested shape.
pub fn document(format: Format) -> Value {
    let commands = commands();
    match format {
        Format::Json | Format::Anthropic | Format::Mcp => {
            let key = if matches!(format, Format::Mcp) {
                "inputSchema"
            } else {
                "input_schema"
            };
            let tools: Vec<Value> = commands
                .iter()
                .map(|command| {
                    json!({
                        "name": command.name,
                        "description": command.description,
                        key: object_schema(command),
                        "examples": command.examples,
                    })
                })
                .collect();
            json!({ "tools": tools })
        }
        Format::Openai => {
            let tools: Vec<Value> = commands
                .iter()
                .map(|command| {
                    json!({
                        "type": "function",
                        "function": {
                            "name": command.name,
                            "description": command.description,
                            "parameters": object_schema(command),
                        },
                    })
                })
                .collect();
            json!({ "tools": tools })
        }
        Format::Gemini => {
            let declarations: Vec<Value> = commands
                .iter()
                .map(|command| {
                    json!({
                        "name": command.name,
                        "description": command.description,
                        "parameters": object_schema(command),
                    })
                })
                .collect();
            json!({ "function_declarations": declarations })
        }
    }
}

/// The parameter object for one command.
fn object_schema(command: &Command) -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "properties": command.properties,
        "required": command.required,
        "additionalProperties": false,
    })
}
