// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Execution profile, output envelope, and structured errors.
//!
//! The CLI has two co-equal readers: a human at a terminal, and an agent paying for
//! tokens. They are rendered independently — the human gets tables and color, the agent
//! gets compact JSON, stable keys, and an error it can act on without guessing.
//!
//! Detection is **presence-based**, not `=1`. Real harnesses export descriptive strings:
//! a live Claude Code session on this machine sets
//! `AI_AGENT=claude-code_2-1-220_agent`, so a detector comparing against `"1"` fails to
//! recognise the agent it is running under. `CI` is the one value-carrying exception.
//!
//! `CLAUDECODE`, `CURSOR_AGENT`, and `GEMINI_CLI` are **informational only**. They name
//! who is calling, and appear in `metadata.invoking_agent`, but they never switch
//! behavior on their own — `AI_AGENT` / `AGENT` / `CI` do that.

use std::io::IsTerminal;

use serde_json::{Value, json};

/// Canonical exit codes. Stable across releases: an agent branches on these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ExitCode {
    /// Completed successfully.
    Ok = 0,
    /// The operation ran and reported a problem with the repository's state.
    Failed = 1,
    /// The invocation itself was wrong: unknown flag, bad value, rejected input.
    Usage = 2,
    /// A file, host, or server that was named does not exist.
    NotFound = 3,
    /// Refused for safety, such as a host that is currently running.
    Refused = 4,
}

/// A failure an agent can act on.
///
/// The `hint` is the whole point: a narrative error leaves an agent to guess flag names
/// and permute arguments until its retry budget runs out. A hint that is a runnable
/// command ends that loop in one step.
#[derive(Debug)]
pub struct Failure {
    /// Stable machine-readable identifier.
    pub code: &'static str,
    /// Process exit code.
    pub exit: ExitCode,
    /// One-line human-readable summary.
    pub message: String,
    /// A command the caller can run next. Never prose, never privileged.
    pub hint: String,
}

impl Failure {
    /// Builds a failure.
    pub fn new(
        code: &'static str,
        exit: ExitCode,
        message: impl Into<String>,
        hint: impl Into<String>,
    ) -> Self {
        Self {
            code,
            exit,
            message: message.into(),
            hint: hint.into(),
        }
    }
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for Failure {}

/// How this invocation should behave and render.
#[derive(Debug, Clone)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent rendering axes resolved from the environment, not a state enum"
)]
pub struct Profile {
    /// Emit JSON rather than prose.
    pub json: bool,
    /// Use ANSI color.
    pub color: bool,
    /// A human is present to answer a prompt.
    pub interactive: bool,
    /// An agent or CI harness is driving.
    pub agent: bool,
    /// Which agent is calling, for telemetry only.
    pub invoking_agent: Option<String>,
}

/// Reads an environment variable, treating empty as unset.
fn present(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

/// `CI` is the one variable whose *value* matters.
fn ci_truthy() -> bool {
    present("CI")
        .is_some_and(|value| !matches!(value.to_ascii_lowercase().as_str(), "false" | "0" | "off"))
}

impl Profile {
    /// Derives the profile from the environment and the explicit flags.
    pub fn detect(force_json: bool, assume_yes: bool) -> Self {
        // Presence-based: any non-empty value means an agent is driving.
        let agent = present("AI_AGENT").is_some() || present("AGENT").is_some() || ci_truthy();

        // Informational only. Named here so it can be reported, never so it can switch
        // behavior -- that distinction is what keeps `CLAUDECODE=1` from silently
        // changing the output format of a human's terminal session.
        let invoking_agent = present("CLAUDECODE")
            .map(|_| "claude-code".to_owned())
            .or_else(|| present("CURSOR_AGENT").map(|_| "cursor".to_owned()))
            .or_else(|| present("GEMINI_CLI").map(|_| "gemini-cli".to_owned()));

        let stdout_tty = std::io::stdout().is_terminal();
        let dumb = present("TERM").is_some_and(|term| term == "dumb");

        let color = present("NO_COLOR").is_none()
            && !dumb
            && !agent
            && (present("FORCE_COLOR").is_some() || stdout_tty);

        Self {
            json: force_json || agent,
            color,
            interactive: !assume_yes && !agent && std::io::stdin().is_terminal() && !dumb,
            agent,
            invoking_agent,
        }
    }

    /// Serializes a value, compact under an agent and pretty for a human.
    ///
    /// Pretty-printing a payload an agent will parse is pure token cost.
    pub fn render(&self, value: &Value) -> String {
        if self.agent || !std::io::stdout().is_terminal() {
            value.to_string()
        } else {
            serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
        }
    }

    /// Wraps a payload in the standard envelope and prints it.
    pub fn emit(&self, command: &str, data: &Value) {
        let mut metadata = serde_json::Map::new();
        metadata.insert("tool".to_owned(), json!("mcpctl"));
        metadata.insert("version".to_owned(), json!(env!("CARGO_PKG_VERSION")));
        metadata.insert("command".to_owned(), json!(command));
        // Omitted entirely when unknown: `"invoking_agent": null` is wasted tokens.
        if let Some(agent) = &self.invoking_agent {
            metadata.insert("invoking_agent".to_owned(), json!(agent));
        }
        let envelope = json!({ "metadata": Value::Object(metadata), "data": data.clone() });
        println!("{}", self.render(&envelope));
    }

    /// Prints a failure in whichever form the caller can use, and returns its exit code.
    pub fn emit_failure(&self, command: &str, failure: &Failure) -> i32 {
        if self.json {
            let mut metadata = serde_json::Map::new();
            metadata.insert("tool".to_owned(), json!("mcpctl"));
            metadata.insert("version".to_owned(), json!(env!("CARGO_PKG_VERSION")));
            metadata.insert("command".to_owned(), json!(command));
            if let Some(agent) = &self.invoking_agent {
                metadata.insert("invoking_agent".to_owned(), json!(agent));
            }
            let envelope = json!({
                "metadata": Value::Object(metadata),
                "error": {
                    "code": failure.code,
                    "exit_code": failure.exit as i32,
                    "message": failure.message,
                    "hint": failure.hint,
                },
            });
            eprintln!("{}", self.render(&envelope));
        } else {
            eprintln!("error: {}", failure.message);
            eprintln!("hint:  {}", failure.hint);
        }
        failure.exit as i32
    }

    /// Machine-readable description of this invocation's resolved behavior.
    ///
    /// `invoking_agent` is omitted rather than emitted as null: every `"field": null` is
    /// a token an agent pays for and learns nothing from.
    pub fn describe(&self) -> Value {
        let mut described = serde_json::Map::new();
        described.insert(
            "profile".to_owned(),
            json!(if self.agent { "agent" } else { "human" }),
        );
        described.insert(
            "format".to_owned(),
            json!(if self.json { "json" } else { "text" }),
        );
        described.insert("color".to_owned(), json!(self.color));
        described.insert("interactive".to_owned(), json!(self.interactive));
        if let Some(agent) = &self.invoking_agent {
            described.insert("invoking_agent".to_owned(), json!(agent));
        }
        Value::Object(described)
    }
}

/// Rejects control characters in a string argument.
///
/// An agent may pass a hallucinated or externally-sourced value; a control character in
/// a host name reaches a terminal, a log, or a file path with effects the caller did not
/// intend. Tab, newline, and carriage return are not valid in any argument this CLI
/// takes, so the whole C0 range is refused.
pub fn reject_control_chars(field: &str, value: &str) -> Result<(), Failure> {
    if let Some(bad) = value.chars().find(|character| character.is_control()) {
        return Err(Failure::new(
            "INVALID_ARGUMENT",
            ExitCode::Usage,
            format!(
                "--{field} contains a control character (U+{:04X})",
                bad as u32
            ),
            "mcpctl schema --json",
        ));
    }
    Ok(())
}
