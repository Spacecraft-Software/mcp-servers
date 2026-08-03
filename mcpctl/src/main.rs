// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Command-line entry point. The work lives in the `mcpctl` library beside it.

use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use mcpctl::runtime::{ExitCode, Failure, Profile, reject_control_chars};
use mcpctl::{check, deploy, fill_keys, render, schema};

/// File whose presence marks the repository root.
const MANIFEST_FILE: &str = "mcp.toml";

#[derive(Debug, Parser)]
#[command(
    name = "mcpctl",
    version,
    about = "Generate and deploy MCP server configs across host applications",
    long_about = None,
)]
struct Cli {
    /// Emit machine-readable JSON instead of human-readable text.
    #[arg(long, global = true)]
    json: bool,

    /// Repository root. Defaults to the nearest ancestor containing `mcp.toml`.
    #[arg(long, global = true, value_name = "DIR")]
    repo: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Verify every host template parses and declares the same set of servers.
    Check,

    /// Generate the host templates from the manifest.
    Render {
        /// Do not write; fail if any template differs from what would be generated.
        #[arg(long)]
        check: bool,
    },

    /// Push the templates into the live host configs under `$HOME`.
    Deploy {
        /// Report what would change without writing anything.
        #[arg(long)]
        dry_run: bool,

        /// Skip all confirmation prompts.
        #[arg(long)]
        yes: bool,

        /// Restrict the operation to a single host, by manifest name.
        #[arg(long, value_name = "NAME")]
        host: Option<String>,

        /// Deploy to a host even while the process that owns its config is running.
        #[arg(long)]
        force: bool,
    },

    /// Substitute API-key placeholders in the live configs.
    FillKeys {
        /// Skip all confirmation prompts.
        #[arg(long)]
        yes: bool,
    },

    /// Print the command surface as JSON Schema for LLM function calling.
    Schema {
        /// Provider wrapping to apply.
        #[arg(long, value_enum, default_value = "json")]
        format: schema::Format,
    },

    /// Report how this invocation resolved: format, color, interactivity, caller.
    Describe,
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();

    // `--yes` on the sub-command also relaxes interactivity, so it has to be known
    // before the profile is built.
    let assume_yes = matches!(
        cli.command,
        Command::Deploy { yes: true, .. } | Command::FillKeys { yes: true }
    );
    let profile = Profile::detect(cli.json, assume_yes);
    let name = command_name(&cli.command);

    match run(&cli, &profile) {
        Ok(code) => std::process::ExitCode::from(code as u8),
        Err(failure) => {
            let code = profile.emit_failure(name, &failure);
            std::process::ExitCode::from(u8::try_from(code).unwrap_or(1))
        }
    }
}

/// Stable command name used in the JSON envelope's metadata.
fn command_name(command: &Command) -> &'static str {
    match command {
        Command::Check => "mcpctl check",
        Command::Render { .. } => "mcpctl render",
        Command::Deploy { .. } => "mcpctl deploy",
        Command::FillKeys { .. } => "mcpctl fill-keys",
        Command::Schema { .. } => "mcpctl schema",
        Command::Describe => "mcpctl describe",
    }
}

/// Dispatches one invocation.
fn run(cli: &Cli, profile: &Profile) -> Result<ExitCode, Failure> {
    // These two need neither a repository nor a manifest, so they are answered before
    // the root is resolved -- an agent must be able to ask what the tool can do from
    // anywhere on the filesystem.
    match &cli.command {
        Command::Schema { format } => {
            println!("{}", profile.render(&schema::document(*format)));
            return Ok(ExitCode::Ok);
        }
        Command::Describe => {
            if profile.json {
                profile.emit("mcpctl describe", &profile.describe());
            } else {
                let described = profile.describe();
                for (key, value) in described.as_object().into_iter().flatten() {
                    println!("{key}: {value}");
                }
            }
            return Ok(ExitCode::Ok);
        }
        _ => {}
    }

    let repo = resolve_repo(cli.repo.as_deref())?;

    match &cli.command {
        Command::Check => check::run(&repo, profile),
        Command::Render { check } => render::run(&repo, *check, profile),
        Command::Deploy {
            dry_run,
            yes,
            host,
            force,
        } => {
            if let Some(name) = host {
                reject_control_chars("host", name)?;
            }
            deploy::run(
                &repo,
                deploy::Options {
                    dry_run: *dry_run,
                    yes: *yes,
                    force: *force,
                },
                host.as_deref(),
                profile,
            )
        }
        Command::FillKeys { yes } => fill_keys::run(&repo, *yes, profile),
        Command::Schema { .. } | Command::Describe => unreachable!("handled above"),
    }
}

/// Resolves and validates the repository root.
///
/// The path is canonicalized before use: `--repo` may carry a hallucinated or
/// externally-sourced value, and a traversal sequence or symlink that resolves outside
/// the intended tree would otherwise be followed silently.
fn resolve_repo(requested: Option<&Path>) -> Result<PathBuf, Failure> {
    let candidate = match requested {
        Some(dir) => dir.to_path_buf(),
        None => find_repo_root()?,
    };

    let resolved = candidate.canonicalize().map_err(|error| {
        Failure::new(
            "NOT_FOUND",
            ExitCode::NotFound,
            format!("`{}` is not readable: {error}", candidate.display()),
            "mcpctl check --repo .",
        )
    })?;

    if !resolved.is_dir() {
        return Err(Failure::new(
            "INVALID_ARGUMENT",
            ExitCode::Usage,
            format!("`{}` is not a directory", resolved.display()),
            "mcpctl check --repo .",
        ));
    }
    Ok(resolved)
}

/// Walks up from the current directory looking for the manifest.
fn find_repo_root() -> Result<PathBuf, Failure> {
    let cwd = std::env::current_dir().map_err(|error| {
        Failure::new(
            "NOT_FOUND",
            ExitCode::NotFound,
            format!("current directory is not readable: {error}"),
            "mcpctl check --repo /path/to/mcp-servers",
        )
    })?;

    for dir in cwd.ancestors() {
        if dir.join(MANIFEST_FILE).is_file() || looks_like_repo(dir) {
            return Ok(dir.to_path_buf());
        }
    }

    Err(Failure::new(
        "NOT_FOUND",
        ExitCode::NotFound,
        format!("no `{MANIFEST_FILE}` in `{}` or any parent", cwd.display()),
        "mcpctl check --repo /path/to/mcp-servers",
    ))
}

/// Heuristic for the pre-manifest layout: `REUSE.toml` next to a known host directory.
fn looks_like_repo(dir: &Path) -> bool {
    dir.join("REUSE.toml").is_file() && dir.join("ClaudeCode").is_dir()
}
