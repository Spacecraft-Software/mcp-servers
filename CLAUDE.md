# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this repo is

A personal collection of **MCP (Model Context Protocol) server configurations**, one config file per host application. There is no source code, no build, and no test suite — the deliverable is the config files themselves. It is a Spacecraft Software-umbrella project (Personal posture, `GPL-3.0-or-later`); its GitHub remote is `Spacecraft-Software/mcp-servers` (migrated from `UnbreakableMJ/mcp-servers`).

The only meaningful validation is that each config is well-formed JSON/TOML/YAML and matches the schema its host expects, and that `reuse lint` stays clean.

## Common Development Tasks

### Validation and Testing

- **Validate config syntax**: Run the validation script to ensure all configs parse correctly:
  ```bash
  python3 .github/validate-configs.py
  ```
  This checks JSON/JSONC/TOML/YAML files and reports any syntax errors.

- **REUSE compliance check**: Ensure all files have proper licensing metadata:
  ```bash
  reuse lint
  ```
  This validates that REUSE.toml covers all files with GPL-3.0-or-later.

- **CI validation**: The GitHub Actions workflow runs both validation and REUSE lint on every push/PR.

### Key Filling and Deployment

- **Fill placeholders**: Use the fill-keys scripts to substitute API key placeholders with real values:
  ```bash
  # POSIX/Bash/Brush
  CONTEXT7_API_KEY=your_key BRAVE_API_KEY=your_key \
    sh bin/fill-keys.sh
  
  # Nushell
  $env:CONTEXT7_API_KEY="your_key" $env:BRAVE_API_KEY="your_key" \
    nu bin/fill-keys.nu
  
  # Ion
  CONTEXT7_API_KEY=your_key BRAVE_API_KEY=your_key \
    ion bin/fill-keys.ion
  ```
  Filled configs are written to `dist/` directory (gitignored).

- **Interactive mode**: Run without env vars to be prompted for missing values:
  ```bash
  sh bin/fill-keys.sh
  ```

- **Custom output directory**: Use `--out` flag to write to a different location:
  ```bash
  sh bin/fill-keys.sh --out /custom/path
  ```

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
3. **Include all ten servers**: Maintain consistency across hosts
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
tool's full personal config, which would carry auth tokens). Each template declares the
full **ten-server superset** (see below). Note this differs from the maintainer's live
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
| `Mimo/mimocode.jsonc` | Mimo Code · `~/.config/mimocode/mimocode.jsonc` | identical to opencode (it's a fork) |
| `Goose/config.yaml` | goose · `~/.config/goose/config.yaml` | YAML `extensions:`; stdio = `type:stdio` + `cmd`/`args`; remote = `type:streamable_http` + `uri` + `headers` |

All files describe the **same logical set of servers** but are not interchangeable —
key names, nesting, and the HTTP transport field all differ per host. When adding or
changing a server, update **every** file in its respective dialect. The fastest way to
get a tool's exact current schema is its own CLI: `claude mcp add`, `codex mcp add`,
`gemini mcp add`, `qwen mcp add`, `grok mcp add` (and `<tool> mcp list` to verify).
Copilot CLI, opencode, mimo, goose, and Kimi have no scriptable add command — hand-edit
those files. Mind the traps: **Qwen uses `httpUrl`** while Gemini uses `url`+`type`;
**Codex uses `http_headers`** while Grok/Kimi use `headers`; Copilot CLI omits the
`mcpServers` wrapper; Gemini needs the separate enablement file and a trusted folder.

## The servers being configured

Every host template declares all ten. Two groups:

**The four "real" servers** (these run in the maintainer's live configs):
- **nixos** — `mcp-nixos` (queries nixpkgs / NixOS options). Antigravity runs the `mcp-nixos` binary directly; everywhere else it's `nix run github:utensils/mcp-nixos --` over stdio.
- **context7** (Upstash) — HTTP, `https://mcp.context7.com/mcp`, needs a `CONTEXT7_API_KEY`. Stored inline under a header (`CONTEXT7_API_KEY` for most hosts; `Authorization: Bearer …` for VS Code, where it comes from a prompted `input`). Placeholder `YOUR_CONTEXT7_API_KEY`.
- **microsoft-learn** — HTTP, `https://learn.microsoft.com/api/mcp`, no auth.
- **crates** — stdio, `crates-mcp` ([crates-mcp](https://crates.io/crates/crates-mcp) via `cargo install`), queries Rust crates from crates.io and docs.rs.

**The six generic servers** (templates-only, placeholders):
- **filesystem** — stdio, `npx -y @modelcontextprotocol/server-filesystem <path>`. Hardcoded path `/spacecraft-software` across all hosts.
- **fetch**, **engram**, **sequential-thinking** — stdio, `npx -y @modelcontextprotocol/server-{fetch,sequential-thinking}` / `engram --db ~/.gemini/engram.db mcp`, no auth.
- **brave-search** — stdio, `npx -y @brave/brave-search-mcp-server` ([brave-search-mcp-server](https://github.com/brave/brave-search-mcp-server)), env `BRAVE_API_KEY=YOUR_BRAVE_API_KEY`.
- **perplexity** — stdio, `npx -y perplexity-mcp`, env `PERPLEXITY_API_KEY=YOUR_PERPLEXITY_API_KEY`.

## Conventions

- Antigravity's file uses **2-space** indentation; VS Code's uses **tabs**. Preserve each file's existing style. Host directory names are **case-sensitive and canonical** (`Antigravity/`, `VSCode/`) — do not reintroduce lowercase `antigravity/` or `.vscode/` variants (a past PR did; they were consolidated).
- Never commit a real secret — templates carry placeholders (`YOUR_CONTEXT7_API_KEY`, `YOUR_BRAVE_API_KEY`, `YOUR_PERPLEXITY_API_KEY`). The filesystem server uses the hardcoded `/spacecraft-software` path. Servers needing placeholders stay inert until filled in locally.
- Templates are the **canonical superset**; the maintainer's live machine runs the three real servers only. When changing a server, update **every** host template in its dialect (and the live config too, for the three real servers).

## Tooling

`bin/fill-keys.{nu,sh,ion}` substitute the three placeholder tokens (`YOUR_CONTEXT7_API_KEY`, `YOUR_BRAVE_API_KEY`, `YOUR_PERPLEXITY_API_KEY`) with values from env vars (prompting for any unset ones when interactive) and write filled copies into a **gitignored `dist/` mirror** — they never edit the tracked templates, so the no-secrets rule holds. There are three parallel ports (Nushell, POSIX/Bash/Brush, Ion) with identical behavior — **change all three together**, plus their shared host-file list (the docs and VS Code's `${input:}` field is deliberately left out). The `.sh`/`.ion` ports shell out to `sd` (literal `-s` mode); the `.nu` port uses native string ops. These are the only executables (`755`); everything else is `644` data. Shell-specific gotchas worth knowing if you edit them: Ion's `test -t` is unreliable (use `tty -s`), Ion eats `-h`/`--help`, and Ion's `test` needs the POSIX `x`-prefix guard for `--`-leading operands.

## Code Architecture and Structure

### High-Level Architecture

The repository follows a **multi-host template pattern** where:

1. **Each host directory** contains a single config file in that host's native format
2. **All configs declare the same ten servers** but in different dialects
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
   - Write filled configs to gitignored `dist/` directory
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
Template Files (tracked) 
  ↓ (fill-keys scripts with env vars)
Filled Configs (dist/, gitignored)
  ↓ (manual copy to live locations)
Host Applications (local config paths)
```

### Important Constraints

1. **No Secrets in Git**: Placeholders ensure no API keys are ever committed
2. **Schema Consistency**: All hosts must declare the same ten servers
3. **Dialect Variations**: Each host uses different field names for the same concepts
4. **REUSE Compliance**: All files licensed GPL-3.0-or-later via REUSE.toml
5. **Signed Commits**: All commits must be cryptographically signed (§6.3)

## Development Workflow

### Typical Session

1. **Make changes**: Edit config templates or add new host support
2. **Validate**: Run `python3 .github/validate-configs.py`
3. **Test fill**: Run `sh bin/fill-keys.sh` with test values
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
# Add line: "NewHost/config.ext|~/.newhost/config.ext"

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
