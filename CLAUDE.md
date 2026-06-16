# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this repo is

A personal collection of **MCP (Model Context Protocol) server configurations**, one config file per host application. There is no source code, no build, and no test suite — the deliverable is the config files themselves. It is a Spacecraft Software-umbrella project (Personal posture, `GPL-3.0-or-later`); its GitHub remote is `Spacecraft-Software/mcp-servers` (migrated from `UnbreakableMJ/mcp-servers`).

The only meaningful validation is that each config is well-formed JSON/TOML/YAML and matches the schema its host expects, and that `reuse lint` stays clean.

## Compliance & posture (Spacecraft Software Standard)

This repo carries the Standard §5.2 posture files and §4.3 REUSE metadata:

- `README.md` (with a Project Posture section), `NOTICE.md`, `CONTRIBUTING.md` — derived from `/spacecraft-software/license/`, tailored to this config repo.
- `LICENSES/GPL-3.0-or-later.txt` — verbatim license text; the root `LICENSE` stays as GitHub's detection pointer.
- `REUSE.toml` — a single `path = "**"` annotation licenses **every** file `GPL-3.0-or-later`. The config templates deliberately carry **no inline SPDX headers** (they are meant to be copied verbatim into users' real host configs, and JSON can't hold comments) — REUSE.toml is the coverage mechanism. When adding any file, it's covered automatically; keep `reuse lint` clean (`nix run nixpkgs#reuse -- lint`).
- Commits must be signed/verified (§6.3) — already configured (SSH ed25519, identity `Mohamed.Hammad@SpacecraftSoftware.org`).
- No `CREDITS.md`: the referenced MCP servers are invoked (e.g. `nix run`) or hit as remote endpoints, not vendored, so §13.3 isn't triggered.

## Layout

Each directory is named after the host application that consumes the file. The
files are **templates**; each holds only the MCP-relevant fragment (never a copy of a
tool's full personal config, which would carry auth tokens). Each template declares the
full **nine-server superset** (see below). Note this differs from the maintainer's live
machine configs, which run only the three real servers — the six generic `npx`/token
servers ship in the templates as placeholders, not in the live configs. Host config
paths are noted below.

| Path (repo) | Host · live path | MCP schema |
|------|------|---------------|
| `Antigravity/mcp_config.json` | Antigravity | top-level `mcpServers`; remote = `serverUrl` + `headers` + `disabled` |
| `VSCode/mcp.json` | VS Code | top-level `servers`; HTTP = `type:"http"` + `url`; secrets via separate `inputs` array |
| `GitHubCopilotCLI/mcp-config.json` | Copilot CLI · `~/.copilot/mcp-config.json` | servers keyed at **top level, no wrapper**; stdio = `command`/`args`/`type:"stdio"`; http = `type:"http"` + `url` + `headers` |
| `ClaudeCode/.mcp.json` | Claude Code · `~/.claude.json` (`mcpServers`) | `mcpServers`; stdio = `type:"stdio"` + `command`/`args`/`env`; http = `type:"http"` + `url` + `headers` |
| `OpenClaude/.mcp.json` | OpenClaude · `~/.openclaude.json` | identical to Claude Code (it's a fork) |
| `Codex/config.toml` | OpenAI Codex · `~/.codex/config.toml` | TOML `[mcp_servers.<id>]`; http = `url` (+ `[mcp_servers.<id>.http_headers]`) |
| `Grok/config.toml` | Grok CLI · `~/.grok/config.toml` | TOML `[mcp_servers.<id>]`; http = `url` + `enabled` (+ `[…​.headers]`) |
| `Kimi/config.toml` | Kimi Code CLI · `~/.kimi-code/config.toml` | TOML `[mcp.client.servers.<name>]`; http = `url` (+ `[…​.headers]`) |
| `Gemini/settings.json` (+ `mcp-server-enablement.json`) | Gemini CLI · `~/.gemini/` | `mcpServers`; http = `url` + `type:"http"` + `headers`; servers must also be enabled in `mcp-server-enablement.json` |
| `Qwen/settings.json` | Qwen Code · `~/.qwen/settings.json` | `mcpServers`; **http = `httpUrl`** (no `type`/`url`) + `headers` |
| `OpenCode/opencode.jsonc` | opencode · `~/.config/opencode/opencode.jsonc` | `mcp` block; local = `type:"local"` + `command:[…]`; remote = `type:"remote"` + `url` + `headers` + `enabled` |
| `Goose/config.yaml` | goose · `~/.config/goose/config.yaml` | YAML `extensions:`; stdio = `type:stdio` + `cmd`/`args`; remote = `type:streamable_http` + `uri` + `headers` |

All files describe the **same logical set of servers** but are not interchangeable —
key names, nesting, and the HTTP transport field all differ per host. When adding or
changing a server, update **every** file in its respective dialect. The fastest way to
get a tool's exact current schema is its own CLI: `claude mcp add`, `codex mcp add`,
`gemini mcp add`, `qwen mcp add`, `grok mcp add` (and `<tool> mcp list` to verify).
Copilot CLI, opencode, goose, and Kimi have no scriptable add command — hand-edit
those files. Mind the traps: **Qwen uses `httpUrl`** while Gemini uses `url`+`type`;
**Codex uses `http_headers`** while Grok/Kimi use `headers`; Copilot CLI omits the
`mcpServers` wrapper; Gemini needs the separate enablement file and a trusted folder.

## The servers being configured

Every host template declares all nine. Two groups:

**The three "real" servers** (these run in the maintainer's live configs too):
- **nixos** — `mcp-nixos` (queries nixpkgs / NixOS options). Antigravity runs the `mcp-nixos` binary directly; everywhere else it's `nix run github:utensils/mcp-nixos --` over stdio.
- **context7** (Upstash) — HTTP, `https://mcp.context7.com/mcp`, needs a `CONTEXT7_API_KEY`. Stored inline under a header (`CONTEXT7_API_KEY` for most hosts; `Authorization: Bearer …` for VS Code, where it comes from a prompted `input`). Placeholder `YOUR_CONTEXT7_API_KEY`.
- **microsoft-learn** — HTTP, `https://learn.microsoft.com/api/mcp`, no auth.

**The six generic servers** (came in via the merged Copilot PRs; templates-only, placeholders):
- **github** — HTTP, `https://api.githubcopilot.com/mcp/`, `Authorization: Bearer YOUR_GITHUB_PAT` (VS Code uses built-in Copilot auth, no header).
- **filesystem** — stdio, `npx -y @modelcontextprotocol/server-filesystem <path>`. Placeholder path `/path/to/your/workspace` (VS Code uses `${workspaceFolder}`).
- **fetch**, **memory**, **sequential-thinking** — stdio, `npx -y @modelcontextprotocol/server-{fetch,memory,sequential-thinking}`, no auth.
- **brave-search** — stdio, `npx -y @modelcontextprotocol/server-brave-search`, env `BRAVE_API_KEY=YOUR_BRAVE_API_KEY`.

## Conventions

- Antigravity's file uses **2-space** indentation; VS Code's uses **tabs**. Preserve each file's existing style. Host directory names are **case-sensitive and canonical** (`Antigravity/`, `VSCode/`) — do not reintroduce lowercase `antigravity/` or `.vscode/` variants (a past PR did; they were consolidated).
- Never commit a real secret — templates carry placeholders (`YOUR_CONTEXT7_API_KEY`, `YOUR_GITHUB_PAT`, `YOUR_BRAVE_API_KEY`) and the `/path/to/your/workspace` path. Servers needing them stay inert until filled in locally.
- Templates are the **canonical superset**; the maintainer's live machine runs the three real servers only. When changing a server, update **every** host template in its dialect (and the live config too, for the three real servers).

## Tooling

`bin/fill-keys.{nu,sh,ion}` substitute the four placeholder tokens (`YOUR_CONTEXT7_API_KEY`, `YOUR_BRAVE_API_KEY`, `YOUR_GITHUB_PAT`, `/path/to/your/workspace`) with values from env vars (prompting for any unset ones when interactive) and write filled copies into a **gitignored `dist/` mirror** — they never edit the tracked templates, so the no-secrets rule holds. There are three parallel ports (Nushell, POSIX/Bash/Brush, Ion) with identical behavior — **change all three together**, plus their shared host-file list (the docs and VS Code's `${input:}`/`${workspaceFolder}` fields are deliberately left out). The `.sh`/`.ion` ports shell out to `sd` (literal `-s` mode); the `.nu` port uses native string ops. These are the only executables (`755`); everything else is `644` data. Shell-specific gotchas worth knowing if you edit them: Ion's `test -t` is unreliable (use `tty -s`), Ion eats `-h`/`--help`, and Ion's `test` needs the POSIX `x`-prefix guard for `--`-leading operands.
