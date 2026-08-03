---
name: mcpctl
description: >
  Generate, verify, and deploy Model Context Protocol (MCP) server configurations across
  13 host applications from a single manifest. Use when adding or changing an MCP server
  for Claude Code, Codex, Gemini CLI, Qwen, opencode, goose, Kimi, Grok, Copilot CLI,
  VS Code, Antigravity, OpenClaude, or Mimo; when host configs have drifted apart; or
  when an API key needs filling or rotating across every host. Commands: mcpctl check
  (hosts agree), render (manifest into templates), deploy (templates into the live
  configs under $HOME), fill-keys (real keys into live configs), schema (JSON Schema for
  function calling), describe (resolved output profile). Triggers include "MCP server",
  "mcp.toml", "mcpctl", "add a server to every agent", "my MCP config is out of sync",
  "rotate my Context7 key".
license: GPL-3.0-or-later
---

# mcpctl

## What it is

One manifest (`mcp.toml`) describing every MCP server, and a Rust binary that renders it
into 13 host dialects, verifies they agree, and pushes them to the live configs under
`$HOME`.

The hosts disagree about nearly every key name — Qwen spells a remote endpoint `httpUrl`,
Codex spells headers `http_headers`, Copilot CLI has no wrapper key, opencode uses
`environment` and a joined command array, goose uses `cmd`/`envs`/`uri`. Those
differences are data in a table, not thirteen code paths.

## Commands

| Command | Purpose |
|---|---|
| `mcpctl check` | Every template parses; every host declares the same servers with the same invocations |
| `mcpctl render [--check]` | Manifest into templates; `--check` fails instead of writing |
| `mcpctl deploy [--dry-run\|--yes] [--host N] [--force]` | Templates into the live configs |
| `mcpctl fill-keys [--yes]` | Real API keys into the live configs; also rotates |
| `mcpctl schema [--format json\|anthropic\|openai\|gemini\|mcp]` | Command surface as JSON Schema |
| `mcpctl describe` | How this invocation resolved: format, color, interactivity, caller |

`--json` on any command gives a `{metadata, data}` envelope; errors give
`{metadata, error{code, exit_code, message, hint}}` where `hint` is a runnable command.

Exit codes: `0` ok, `1` the repository or configs are in a bad state, `2` bad invocation,
`3` not found, `4` refused for safety.

## The workflow that matters

```sh
$EDITOR mcp.toml     # 1. change a server here, never in a host directory
mcpctl render        # 2. regenerate all 13 templates
mcpctl deploy        # 3. push to the machine  <- the step that is easy to forget
# 4. restart every host; a failed MCP server is not retried in-process
```

Step 3 exists because nothing reads the templates. A change that stops at step 2 is
correct in the repository and absent from every host, with a clean `git status`.

## Safety

`deploy` writes files owned by other tools, so it replaces only the MCP block (about 60
lines of `~/.claude.json`'s 6,835), preserves servers it does not manage byte-for-byte,
never writes a placeholder where a real key belongs, re-parses before writing, writes by
rename over a temporary, backs up to `~/.mcp-backup/<timestamp>/`, refuses a host whose
process is running, and defaults to `--dry-run` with no terminal or under an agent.

## Requirements

A Rust toolchain (`nix develop` provides one) and, for `deploy`/`fill-keys`, the host
config files themselves. No network access is needed.

To put `mcpctl` on `PATH` rather than running it out of `target/`:

```sh
nix profile add /spacecraft-software/mcp-servers#mcpctl
```

The install is pinned at that moment — re-run it after changing `mcpctl/` or `mcp.toml`,
and prefer `cargo build --release` while iterating.

Running `deploy` from inside an agent session reports rather than writes: it is a dry run
under `CLAUDECODE`/`CI`, and a host whose process is running is refused outright. Writing
to a live config is a human action from a plain shell.
