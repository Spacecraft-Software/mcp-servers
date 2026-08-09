<!--
SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# AGENTS.md

Project-specific invariants for agents working in this repository. This file does not
restate the Spacecraft Software CLI Standard or the Rust guidelines — load those skills
instead. What follows is only what is true *here*.

## What this repository is

MCP server configurations for 13 host applications. `mcp.toml` is the source of truth;
everything under a host directory is **generated**. `mcpctl/` is a Rust binary that
generates, checks, and deploys them.

## Invariants — breaking any of these is a bug

1. **Never edit a file under a host directory.** `Antigravity/`, `Codex/`, `Goose/`, and
   the ten others are generated. The next `mcpctl render` reverts your change and CI
   fails on the difference. Edit `mcp.toml`.
2. **Never put a real API key in a tracked file.** Templates carry placeholders
   (`YOUR_CONTEXT7_API_KEY` and friends). Real keys exist only in `$HOME`, written by
   `mcpctl fill-keys`.
3. **A change is not done until it is deployed.** Nothing reads the templates — no host
   loads a file from this repository. `render` updates the repo; `deploy` updates the
   machine. Stopping after `render` leaves every host on the old configuration with a
   clean `git status`. This has happened twice: commit `cc1fcfb` and the `perplexity`
   server, which sat in all 13 templates and 6 of 15 live configs.
4. **Live configs belong to their tools.** `deploy` replaces only the MCP block, keeps
   servers it does not manage byte-for-byte, re-parses before writing, and backs up
   first. Do not weaken any of those; see `CLAUDE.md` § Deploying safely for why each
   one exists.
5. **Emitters must write empty collections**, not skip them. `available_tools: []`
   belongs to goose, and dropping it is silent data loss in a file we do not own.
6. **`serde_json` and `toml` are built with `preserve_order`.** Without it, env blocks
   alphabetize themselves on every render. Use `shift_remove`, never `remove`, on these
   maps — `remove` is a swap-remove under `preserve_order` and shuffles the block.
7. **Commits are signed** (Standard §6.3) and pushed only to `Spacecraft-Software` or
   `UnbreakableMJ` (§6.4).

## Build, lint, test

The toolchain comes from `nix develop`. The `rustup` toolchains on this machine are
linked against `/lib64/ld-linux-x86-64.so.2` and cannot execute on NixOS — `cargo`
resolves on `PATH` and then fails with "cannot execute: required file not found".

```sh
nix develop
cargo fmt --manifest-path mcpctl/Cargo.toml --check
cargo clippy --manifest-path mcpctl/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path mcpctl/Cargo.toml
cargo build --release --manifest-path mcpctl/Cargo.toml

./mcpctl/target/release/mcpctl check          # hosts agree with each other
./mcpctl/target/release/mcpctl render --check  # templates agree with the manifest
reuse lint
```

Both `check` and `render --check` are CI gates. Clippy runs with the full pedantic and
restriction set at `-D warnings`.

## Test the risky paths by running them

`deploy` and `fill-keys` write to files the user and their tools own. Reading the diff
does not tell you whether they are correct. Exercise them against a copy of the real
configs:

```sh
mkdir -p /tmp/homesim
cp -r ~/.codex ~/.gemini ~/.config/goose /tmp/homesim/   # and the rest
HOME=/tmp/homesim ./mcpctl/target/release/mcpctl deploy --yes --force
```

Every serious bug in this tool was found this way and none was visible in the diff: 13
goose extensions silently losing `available_tools: []`, placeholders being written into
live configs as credentials, and `deploy` and `fill-keys` reordering each other's output
forever. All three exited 0.

## Agent-facing behavior

`mcpctl` detects agents by **presence**, not by `=1` — a live Claude Code session exports
`AI_AGENT=claude-code_2-1-220_agent`. `AI_AGENT`, `AGENT`, and a truthy `CI` switch the
profile to agent: JSON output, no color, non-interactive. `CLAUDECODE`, `CURSOR_AGENT`,
and `GEMINI_CLI` are **informational only** and appear in `metadata.invoking_agent`; they
must never become behavioral switches.

Useful entry points when you do not know the surface:

```sh
mcpctl describe --json     # how this invocation resolved
mcpctl schema --json       # every command as JSON Schema Draft 2020-12
```

Every error carries a runnable `hint`. If you hit one, run the hint rather than guessing
at flags.

## Adding a host

Two edits, and the split matters:

- `mcpctl/src/dialect.rs` — one row in `HOSTS`, the host's *mechanics*: which key holds a
  URL, whether env vars are `env`/`environment`/`envs`, split or joined command,
  indentation, `type` values, how "enabled" is spelled.
- `mcp.toml` — one `[[hosts]]` entry, the *content*: template path, live config paths,
  flags.

The manifest validates that both sides agree; a host in one and not the other is an
error, not a silent omission.

## Adding a server

One edit for a server that needs no credential: a `[[servers]]` entry in `mcp.toml`. Its
position in the file is the order every host lists servers in, and `tests/render.rs`
derives its parity expectation from `resolve_for`, so no test changes.

A server that needs a credential takes **three** more edits, and the third is easy to miss:

- `[[secrets]]` in `mcp.toml` — the `placeholder` ↔ `env` pair `fill-keys` resolves.
- `[servers.overrides.VSCode]` — VS Code cannot hold a literal secret; it wants
  `${input:<ENV>}`. An override replaces `env`/`headers` **wholesale** rather than merging,
  so every other variable in the block has to be repeated or it silently disappears on that
  host alone.
- `ACCEPTED` in `mcpctl/src/check.rs` — that override is a per-host difference, and
  `mcpctl check` reports any undeclared difference as drift. Without a `Variation` row for
  the new `VSCode`/`<server>` pair, `check` fails, and it is a CI gate.

Then the usual: `mcpctl render`, the gates, `deploy`, `fill-keys` (invariant 3).

Two things worth measuring rather than assuming, both learned the hard way (see `crates`
and `fetch` in `mcp.toml`): whether the server writes its logs to **stdout**, which
poisons the JSON-RPC stream and hard-fails strict clients, and whether its dependencies
need pinning. Probe stdout before trusting it:

```sh
tail -f requests.jsonl | timeout 10 <server command> > out.txt 2> err.txt
head -c 2 out.txt    # must be `{"` — anything else means logs are on the wrong stream
```

## Omitting a server from one host

`omit = true` in `[servers.overrides.<Host>]` means that host does not carry the server
at all — no entry is emitted anywhere. Unlike the `VSCode` overrides above there is **no**
`check.rs` row to remember: `check` reads omissions straight from the manifest and reports
them under `omitted`, separately from the `missing` list that fails the gate.

`enabled = false` is a different mechanism and not a substitute. A disabled server is
still written into the host's config and merely switched off. `context7` and
`microsoft-learn` are omitted for Claude Code because it hides a claude.ai connector whose
URL a locally configured server already claims, and it is *presence* of the URL that does
the hiding.

Invariant 3 bites harder here. `render` drops the entry from the template, but `deploy`
then sees the live entry as a **stray** and preserves it byte-for-byte, and `--yes` forces
non-interactive mode so it never prunes. Finish the job with `mcpctl deploy --host <Host>`
from a real terminal — no `--yes`, host not running — and answer `y` to each removal.

`mcpctl` only ever touches the **top-level** `mcpServers` block of `~/.claude.json`.
Claude Code also stores per-project servers under `projects.<path>.mcpServers`; `splice`
never looks there, so those copies are invisible to `deploy` and need
`claude mcp remove -s local`.
