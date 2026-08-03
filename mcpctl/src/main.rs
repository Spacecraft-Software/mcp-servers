// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Command-line entry point. The work lives in the `mcpctl` library beside it.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use mcpctl::{check, deploy, fill_keys, render};

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
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let repo = match cli.repo {
        Some(dir) => dir,
        None => find_repo_root().context("could not locate the repository root")?,
    };

    match cli.command {
        Command::Check => check::run(&repo, cli.json),
        Command::Render { check } => render::run(&repo, check, cli.json),
        Command::Deploy {
            dry_run,
            yes,
            host,
            force,
        } => deploy::run(
            &repo,
            deploy::Options {
                dry_run,
                yes,
                force,
                as_json: cli.json,
            },
            host.as_deref(),
        ),
        Command::FillKeys { yes } => fill_keys::run(&repo, yes, cli.json),
    }
}

/// Walks up from the current directory looking for the manifest.
///
/// Falls back to a directory that merely looks like this repository (a `REUSE.toml`
/// beside host template directories), so `check` remains usable before the manifest
/// exists.
fn find_repo_root() -> Result<PathBuf> {
    let cwd = std::env::current_dir().context("current directory is not readable")?;
    for dir in cwd.ancestors() {
        if dir.join(MANIFEST_FILE).is_file() || looks_like_repo(dir) {
            return Ok(dir.to_path_buf());
        }
    }
    bail!(
        "no `{MANIFEST_FILE}` found in `{}` or any parent directory; pass --repo",
        cwd.display()
    )
}

/// Heuristic for the pre-manifest layout: `REUSE.toml` next to a known host directory.
fn looks_like_repo(dir: &Path) -> bool {
    dir.join("REUSE.toml").is_file() && dir.join("ClaudeCode").is_dir()
}
