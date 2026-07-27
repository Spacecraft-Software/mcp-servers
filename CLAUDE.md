# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this repo is

A personal collection of **MCP (Model Context Protocol) server configurations**, one config file per host application. There is no source code, no build, and no test suite — the deliverable is the config files themselves. It is a Spacecraft Software-umbrella project (Personal posture, `GPL-3.0-or-later`); its GitHub remote is `Spacecraft-Software/mcp-servers` (migrated from `UnbreakableMJ/mcp-servers`).

The only meaningful validation is that each config is well-formed JSON/TOML/YAML and matches the schema its host expects, and that `reuse lint` stays clean.

## Common Development Tasks

### Validation and Testing

- **Validate config syntax**: `.github/validate-configs.py` walks the tree (skipping `.git`)
  and parses every `.json` / `.jsonc` / `.toml` / `.yaml` / `.yml` file by extension, exiting
  non-zero and listing each malformed one. New host templates are picked up automatically —
  there is no list to maintain.

  It imports **PyYAML**, which the system Python on this machine does not carry, so a bare
  `python3 .github/validate-configs.py` fails with `ModuleNotFoundError: No module named
  'yaml'`. Supply it ephemerally instead:
  ```bash
  uv run --no-project --with pyyaml python .github/validate-configs.py
  ```
  Or via nix, if you prefer:
  ```bash
  nix shell --impure --expr '(import <nixpkgs> {}).python3.withPackages(ps: [ps.pyyaml])' \
    --command python3 .github/validate-configs.py
  ```
  The plain `python3 .github/validate-configs.py` is correct anywhere PyYAML is already
  present — that is what CI runs, after a `pip install`. `tomllib` is stdlib on Python 3.11+
  and needs nothing.

- **REUSE compliance check**: Ensure all files have proper licensing metadata:
  ```bash
  reuse lint
  ```
  This validates that REUSE.toml covers all files with GPL-3.0-or-later. `reuse` is on PATH
  here; `nix run nixpkgs#reuse -- lint` works where it is not.

- **CI validation**: `.github/workflows/ci.yml` runs both on every push to `main` and every
  PR — Python 3.12, `pip install reuse pyyaml`, then `reuse lint` and the validation script.

### Key Filling and Deployment

The fill-keys scripts substitute the three placeholder tokens
(`YOUR_CONTEXT7_API_KEY`, `YOUR_BRAVE_API_KEY`, `YOUR_PERPLEXITY_API_KEY`) with real
values **directly in the live config files** under `$HOME`. They never write to `dist/`,
never take an output path, and never touch the tracked templates — which is what keeps the
no-secrets rule intact. Requires [`sd`](https://github.com/chmln/sd)
(`cargo install sd`, or `nix run nixpkgs#sd`).

- **Interactive (the normal path)**: run with no arguments. Missing keys are prompted for
  with echo off (blank input skips that key), then each live config gets its own
  `Fill keys in <label> (<path>)? [Y/n]` prompt, so you choose which hosts are touched:
  ```bash
  sh bin/fill-keys.sh          # POSIX / Bash / Brush / dash / ash
  nu bin/fill-keys.nu          # Nushell
  ion bin/fill-keys.ion        # Ion
  ```

- **Keys from the environment**: any key already exported is used without a prompt; the
  per-file Y/N prompts still apply.
  ```bash
  # POSIX / Bash / Brush — also the right way to launch the nu and ion ports
  CONTEXT7_API_KEY=… BRAVE_API_KEY=… PERPLEXITY_API_KEY=… sh bin/fill-keys.sh

  # from inside Nushell
  $env.CONTEXT7_API_KEY = "…"; nu bin/fill-keys.nu
  ```

- **Unattended / CI**: `--yes` skips every Y/N prompt. Prompting also switches itself off
  whenever stdin is not a TTY or `CI` / `CLAUDECODE` is set, so an agent-driven run never
  blocks on a prompt — supply the keys as env vars in that case.
  ```bash
  sh bin/fill-keys.sh --yes
  ```

The only flags are `--yes` and `-h`/`--help` (the Nushell port also accepts `-y`). Ion eats
`-h`/`--help`, so its usage text is best-effort (see the note in `bin/fill-keys.ion`). The
live-config path list lives in all three scripts and must stay in sync across them — see
[Tooling](#tooling).

### Adding or Modifying Servers

When adding a new server or modifying an existing one:

1. **Update all host templates**: Each server must be declared in every host's config file using that host's specific schema dialect
2. **Follow schema conventions**: Pay attention to host-specific field names (e.g., Qwen uses `httpUrl`, others use `url`)
3. **Preserve indentation**: Antigravity uses 2-space, VSCode uses tabs
4. **Use placeholders for secrets**: Never commit real API keys
5. **Update documentation**: Modify README.md server table and CLAUDE.md schema table

### Adding a New Host

To add support for a new MCP-capable tool:

1. **Create new directory**: Named after the host tool (case-sensitive)
2. **Add config template**: Follow the host's schema (use `host mcp add` if available)
3. **Include all twelve servers**: Maintain consistency across hosts
4. **Add to fill-keys scripts**: Update the host-file list in all three shell scripts
5. **Update README**: Add to the supported hosts table
6. **Update CLAUDE.md**: Add to the layout table with schema notes

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
tool's full personal config, which would carry auth tokens). Each template declares the full **twelve-server superset** (see below). Note this differs from the maintainer's live
machine configs, which run only the five real servers — the seven generic `npx`/token
servers ship in the templates as placeholders, not in the live configs. Host config
paths are noted below.

| Path (repo) | Host · live path | MCP schema |
|------|------|---------------|
| `Antigravity/mcp_config.json` | Antigravity | top-level `mcpServers`; remote = `serverUrl` + `headers` + `disabled` |
| `VSCode/mcp.json` | VS Code | top-level `servers`; HTTP = `type:"http"` + `url`; secrets via separate `inputs` array |
| `GitHubCopilotCLI/mcp-config.json` | Copilot CLI · `~/.copilot/mcp-config.json` | servers keyed at **top level, no wrapper**; stdio = `command`/`args`/`type:"stdio"`; http = `type:"http"` + `url` + `headers` |
| `ClaudeCode/.mcp.json` + `ClaudeCode/settings.json` | Claude Code · `~/.claude.json` (`mcpServers`) + `~/.claude/settings.json` | `mcpServers`; stdio = `type:"stdio"` + `command`/`args`/`env`; http = `type:"http"` + `url` + `headers`. `.mcp.json` has **no per-server disable field** — `sequential-thinking` is declared but shipped off by default via `settings.json`'s `disabledMcpjsonServers` |
| `OpenClaude/.mcp.json` | OpenClaude · `~/.openclaude.json` | identical to Claude Code (it's a fork) |
| `Codex/config.toml` | OpenAI Codex · `~/.codex/config.toml` | TOML `[mcp_servers.<id>]`; http = `url` (+ `[mcp_servers.<id>.http_headers]`) |
| `Grok/config.toml` | Grok CLI · `~/.grok/config.toml` | TOML `[mcp_servers.<id>]`; http = `url` + `enabled` (+ `[…​.headers]`) |
| `Kimi/mcp.json` | Kimi Code CLI · `~/.kimi-code/mcp.json` | top-level `mcpServers`; stdio = `command`/`args`/`env`; http = `url` (+ `headers`, or Kimi's own `bearerTokenEnvVar`). **Not `config.toml`** — Kimi's `config.toml` has no MCP schema at all (confirmed against the installed binary); it's scoped to model/provider/loop-control/hooks only. Kimi resolves MCP servers from three tiers, later overriding earlier on name collision: user-global `~/.kimi-code/mcp.json`, project-root `<project>/.mcp.json` (Claude-compatible), project-local `<cwd>/.kimi-code/mcp.json` |
| `Gemini/settings.json` (+ `mcp-server-enablement.json`) | Gemini CLI · `~/.gemini/` | `mcpServers`; http = `url` + `type:"http"` + `headers`; servers must also be enabled in `mcp-server-enablement.json` |
| `Qwen/settings.json` | Qwen Code · `~/.qwen/settings.json` | `mcpServers`; **http = `httpUrl`** (no `type`/`url`) + `headers` |
| `OpenCode/opencode.jsonc` | opencode · `~/.config/opencode/opencode.jsonc` | `mcp` block; local = `type:"local"` + `command:[…]`; remote = `type:"remote"` + `url` + `headers` + `enabled`. `enabled` is **optional and defaults to on** — only an explicit `enabled: false` disables a server. Optional per-server `timeout` is in **ms** (default 30000) and is the only knob on the *connect* path; `experimental.mcp_timeout` covers requests only |
| `Mimo/mimocode.jsonc` | Mimo Code · `~/.config/mimocode/mimocode.jsonc` | identical to opencode (it's a fork) — same `enabled`/`timeout` semantics, same merge order |
| `Goose/config.yaml` | goose · `~/.config/goose/config.yaml` | YAML `extensions:`; stdio = `type:stdio` + `cmd`/`args`; remote = `type:streamable_http` + `uri` + `headers` |

All files describe the **same logical set of servers** but are not interchangeable —
key names, nesting, and the HTTP transport field all differ per host. When adding or
changing a server, update **every** file in its respective dialect. The fastest way to
get a tool's exact current schema is its own CLI: `claude mcp add`, `codex mcp add`,
`gemini mcp add`, `qwen mcp add`, `grok mcp add` (and `<tool> mcp list` to verify).
Copilot CLI, opencode, mimo, goose, and Kimi have no scriptable add command — hand-edit
those files (Kimi does have a built-in `/mcp-config` skill inside the Kimi Code TUI
itself, just no non-interactive CLI verb). Mind the traps: **Qwen uses `httpUrl`** while
Gemini uses `url`+`type`; **Codex uses `http_headers`** while Grok/Kimi use `headers`;
Copilot CLI omits the `mcpServers` wrapper; Gemini needs the separate enablement file
and a trusted folder; **Kimi's MCP config lives in `mcp.json`, never `config.toml`**
despite Kimi using TOML for everything else it configures.

### opencode / Mimo: merge order and disable semantics

Both hosts **deep-merge, they do not first-match**: `config.json` → `<host>.json` →
`<host>.jsonc`, applied in that order, so **the `.jsonc` wins key-for-key**. Keeping both a
`.json` and a `.jsonc` in the same config dir is a footgun — a duplicated server in the
`.json` looks live but is dead, and a stale API key there can silently shadow nothing at
all while you debug the wrong file. Keep exactly one config file per host; the `.jsonc` is
the right one to keep, because it is also where the TUI/CLI writes config edits
(`opencode mcp add` and friends resolve to the first existing of
`<host>.jsonc`, `<host>.json`, `config.json`) and where `bin/fill-keys.*` fills keys.

Two more behaviours worth knowing when a server looks "disabled by default":

- It isn't a default. `MCP.create` short-circuits **only** on an explicit `enabled === false`;
  an absent `enabled` means enabled, for both `type:"local"` and `type:"remote"`.
- The in-TUI `/mcp` panel (`mod+;`) toggles are **in-memory only** — `MCP.disconnect`
  mutates a map that is rebuilt from the config file on every launch, and nothing MCP-related
  is persisted to `opencode.db` or `~/.cache/opencode/`. The config file is the only durable
  lever.

Remote servers that fail OAuth land in `needs_auth` / `needs_client_registration` with only
a transient toast — that is *not* the same state as `disabled`, and no config change causes it.

## The servers being configured

Every host template declares all twelve. Two groups:

**The five "real" servers** (these run in the maintainer's live configs):
- **nixos** — `mcp-nixos` (queries nixpkgs / NixOS options). Antigravity runs the `mcp-nixos` binary directly; everywhere else it's `nix run github:utensils/mcp-nixos --` over stdio.
- **context7** (Upstash) — HTTP, `https://mcp.context7.com/mcp`, needs a `CONTEXT7_API_KEY`. Stored inline under a header (`CONTEXT7_API_KEY` for most hosts; `Authorization: Bearer …` for VS Code, where it comes from a prompted `input`). Placeholder `YOUR_CONTEXT7_API_KEY`.
- **microsoft-learn** — HTTP, `https://learn.microsoft.com/api/mcp`, no auth.
- **crates** — stdio, `crates-mcp` ([crates-mcp](https://crates.io/crates/crates-mcp) via `cargo install`), queries Rust crates from crates.io and docs.rs.
- **bravais-cli** — stdio, `bravais-cli mcp`, queries tool preferences and rewrites shell commands.

**The seven generic servers** (templates-only, placeholders):
- **filesystem** — stdio, `npx -y @modelcontextprotocol/server-filesystem <path>`. Hardcoded path `/spacecraft-software` across all hosts.
- **fetch**, **engram**, **sequential-thinking** — stdio, `npx -y @modelcontextprotocol/server-{fetch,sequential-thinking}` / `engram --db ~/.gemini/engram.db mcp`, no auth.
- **brave-search** — stdio, `npx -y @brave/brave-search-mcp-server` ([brave-search-mcp-server](https://github.com/brave/brave-search-mcp-server)), env `BRAVE_API_KEY=YOUR_BRAVE_API_KEY`.
- **perplexity** — stdio, `npx -y perplexity-mcp`, env `PERPLEXITY_API_KEY=YOUR_PERPLEXITY_API_KEY`.
- **terminal** — stdio, `npx -y mcp-server-terminal` ([mcp-server-terminal](https://github.com/aybelatchane/mcp-server-terminal)), env `RUST_LOG=error`, `NO_COLOR=1`, `TERM=dumb` (keeps TUI output plain for the agent); no auth.

## Conventions

- Antigravity's file uses **2-space** indentation; VS Code's uses **tabs**. Preserve each file's existing style. Host directory names are **case-sensitive and canonical** (`Antigravity/`, `VSCode/`) — do not reintroduce lowercase `antigravity/` or `.vscode/` variants (a past PR did; they were consolidated).
- Never commit a real secret — templates carry placeholders (`YOUR_CONTEXT7_API_KEY`, `YOUR_BRAVE_API_KEY`, `YOUR_PERPLEXITY_API_KEY`). The filesystem server uses the hardcoded `/spacecraft-software` path. Servers needing placeholders stay inert until filled in locally.
- Templates are the **canonical superset**; the maintainer's live machine runs the three real servers only. When changing a server, update **every** host template in its dialect (and the live config too, for the three real servers).

## Tooling

`bin/fill-keys.{nu,sh,ion}` substitute the three placeholder tokens (`YOUR_CONTEXT7_API_KEY`, `YOUR_BRAVE_API_KEY`, `YOUR_PERPLEXITY_API_KEY`) with values from env vars and write them **directly into the live config files** — they never touch the tracked templates, so the no-secrets rule holds. Before modifying each file, the script presents a Y/N prompt (skippable with `--yes`). The `.sh`/`.ion` ports shell out to `sd` (literal `-s` mode); the `.nu` port uses native string ops. There are three parallel ports (Nushell, POSIX/Bash/Brush, Ion) with identical behavior — **change all three together** (their live-config path lists must stay in sync). Shell-specific gotchas worth knowing if you edit them: Ion's `test -t` is unreliable (use `tty -s`), Ion eats `-h`/`--help`, and Ion's `test` needs the POSIX `x`-prefix guard for `--`-leading operands.

## Code Architecture and Structure

### High-Level Architecture

The repository follows a **multi-host template pattern** where:

1. **Each host directory** contains a single config file in that host's native format
2. **All configs declare the same twelve servers** but in different dialects
3. **Templates use placeholders** for secrets that get filled at deployment time
4. **No real secrets are committed** - placeholders ensure safety

### Key Components

1. **Host Config Templates** (`*/*.json`, `*/*.toml`, `*/*.yaml`):
   - JSON/TOML/YAML files following each host's MCP schema
   - Contain placeholder values for API keys and paths
   - Organized by host tool name in separate directories

2. **Key Filling Scripts** (`bin/fill-keys.*`):
   - Three parallel implementations for different shells
   - Substitute placeholders with real values from environment variables
   - Fill directly into live config files with per-file Y/N confirmation
   - Never modify tracked templates

3. **Validation System** (`.github/validate-configs.py`):
   - Parses all config files to ensure valid syntax
   - Handles JSONC (with comment stripping), JSON, TOML, YAML
   - Runs automatically in CI on every push/PR

4. **Documentation** (`README.md`, `CLAUDE.md`, `CONTRIBUTING.md`):
   - Comprehensive guides for usage and contribution
   - Schema reference tables for each host
   - Project posture and licensing information

### Data Flow

```
Template Files (tracked)  →  reference for what each host config should look like
                                ↓
Live Config Files (untracked, ~/.gemini/, ~/.config/opencode/, etc.)
  ↓ (fill-keys scripts with env vars + Y/N confirmation per file)
Placeholders replaced in-place — keys never land in the repo
```

### Important Constraints

1. **No Secrets in Git**: Placeholders ensure no API keys are ever committed
2. **Schema Consistency**: All hosts must declare the same twelve servers
3. **Dialect Variations**: Each host uses different field names for the same concepts
4. **REUSE Compliance**: All files licensed GPL-3.0-or-later via REUSE.toml
5. **Signed Commits**: All commits must be cryptographically signed (§6.3)

## Development Workflow

### Typical Session

1. **Make changes**: Edit config templates or add new host support
2. **Validate**: Run `python3 .github/validate-configs.py`
3. **Fill keys**: Run `sh bin/fill-keys.sh --yes` with env vars set
4. **Check REUSE**: Run `reuse lint`
5. **Commit**: Use signed commits with conventional commit messages
6. **Push**: CI will validate on GitHub

### Adding a New Host Example

```bash
# 1. Create new host directory
mkdir -p NewHost

# 2. Add config template following host's schema
# Use the host's own `mcp add` command if available, or manual creation

# 3. Add to fill-keys scripts
# Edit bin/fill-keys.sh, bin/fill-keys.nu, bin/fill-keys.ion
# Add entry in the live-config list: label and path under $HOME

# 4. Update documentation
# Edit README.md (supported hosts table)
# Edit CLAUDE.md (layout table and schema notes)

# 5. Validate
python3 .github/validate-configs.py
reuse lint
```

### Common Pitfalls

1. **Schema Mismatches**: Qwen uses `httpUrl`, others use `url` + `type`
2. **Indentation**: VSCode uses tabs, Antigravity uses 2 spaces
3. **Wrapper Keys**: Copilot CLI has no `mcpServers` wrapper, others do
4. **Header Names**: Codex uses `http_headers`, others use `headers`
5. **Placeholder Consistency**: Must use exact placeholder strings across all files
