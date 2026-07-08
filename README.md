# mcp-servers

Per-tool **MCP (Model Context Protocol) server configurations** for the coding agents
and editors I use. Each directory holds the MCP config fragment for one host, in that
host's own dialect. They all wire up the same ten servers:

| Server | Transport | Endpoint / command | Auth |
|--------|-----------|--------------------|------|
| **nixos** | stdio | `nix run github:utensils/mcp-nixos --` ([mcp-nixos](https://github.com/utensils/mcp-nixos)) — nixpkgs / NixOS options | none |
| **context7** | http | `https://mcp.context7.com/mcp` — Upstash Context7 library docs | `CONTEXT7_API_KEY` |
| **microsoft-learn** | http | `https://learn.microsoft.com/api/mcp` — Microsoft Learn docs | none |
| **filesystem** | stdio | `npx -y @modelcontextprotocol/server-filesystem <path>` — sandboxed file access | none (set a path) |
| **fetch** | stdio | `uvx mcp-server-fetch` — fetch live web content | none |
| **engram** | stdio | `engram --db ~/.gemini/engram.db mcp` — shared verbatim chat memory | none |
| **brave-search** | stdio | `npx -y @brave/brave-search-mcp-server` — web, local, news, image, video search | `BRAVE_API_KEY` |
| **perplexity** | stdio | `npx -y perplexity-mcp` — Perplexity search | `PERPLEXITY_API_KEY` |
| **sequential-thinking** | stdio | `npx -y @modelcontextprotocol/server-sequential-thinking` — step-by-step reasoning | none |
| **crates** | stdio | `crates-mcp` ([crates-mcp](https://crates.io/crates/crates-mcp) via `cargo install`) — Rust crate search and docs | none |

The `npx`-based servers need Node.js 18+. `brave-search`, `perplexity`, and `filesystem`
need a token or path filled in before they work (see Notes).

## Supported hosts

| Directory | Host | Live config path |
|-----------|------|------------------|
| `Antigravity/` | Antigravity | `~/.gemini/config/mcp_config.json` (CLI)<br>`~/.gemini/antigravity/mcp_config.json` (2.0)<br>`~/.gemini/antigravity-ide/mcp_config.json` (IDE) |
| `VSCode/` | VS Code | `.vscode/mcp.json` |
| `GitHubCopilotCLI/` | GitHub Copilot CLI | `~/.copilot/mcp-config.json` |
| `ClaudeCode/` | Claude Code | `~/.claude.json` |
| `OpenClaude/` | OpenClaude | `~/.openclaude.json` |
| `Codex/` | OpenAI Codex | `~/.codex/config.toml` |
| `Grok/` | Grok CLI | `~/.grok/config.toml` |
| `Kimi/` | Kimi Code CLI | `~/.kimi-code/config.toml` |
| `Gemini/` | Gemini CLI | `~/.gemini/settings.json` |
| `Qwen/` | Qwen Code | `~/.qwen/settings.json` |
| `OpenCode/` | opencode | `~/.config/opencode/opencode.jsonc` |
| `Mimo/` | Mimo Code | `~/.config/mimocode/mimocode.jsonc` |
| `Goose/` | goose | `~/.config/goose/config.yaml` |

## Notes

- Files are **templates** with placeholders — replace these locally, never commit real
  values: `YOUR_CONTEXT7_API_KEY`, `YOUR_BRAVE_API_KEY`, `YOUR_PERPLEXITY_API_KEY`. The
  `filesystem` server uses the hardcoded path `/spacecraft-software`. Until placeholders
  are filled in, those servers won't connect (the other servers work as-is). VS Code
  instead prompts for the Context7, Brave, and Perplexity keys via its `inputs`
  mechanism.
- Schemas differ per host (e.g. Qwen uses `httpUrl`, Codex uses `http_headers`, Copilot
  CLI omits the `mcpServers` wrapper). See `CLAUDE.md` for the full per-host schema table.

## Filling in your keys

`bin/fill-keys.*` substitutes the placeholders with your real values **directly in the
live config files** — the tracked templates are never modified, so no secret is ever
committed. Before modifying each file, the script asks for confirmation (use `--yes` to
auto-approve). The same tool is provided for three shells (pick whichever you run); all
behave identically:

| Script | Shell | Notes |
|--------|-------|-------|
| `bin/fill-keys.nu` | Nushell | no external deps (native string ops) |
| `bin/fill-keys.sh` | POSIX sh / Bash / Brush | needs [`sd`](https://github.com/chmln/sd) |
| `bin/fill-keys.ion` | Ion | needs `sd`; Ion eats `-h`/`--help` itself, so see the file header for usage |

| Env var | Fills | Server |
|---------|-------|--------|
| `CONTEXT7_API_KEY` | `YOUR_CONTEXT7_API_KEY` | context7 |
| `BRAVE_API_KEY` | `YOUR_BRAVE_API_KEY` | brave-search |
| `PERPLEXITY_API_KEY` | `YOUR_PERPLEXITY_API_KEY` | perplexity |

```sh
# Provide values via env vars (any you omit are prompted for, or left as placeholders
# when run non-interactively). Use whichever script matches your shell:
CONTEXT7_API_KEY=ctx7sk-... BRAVE_API_KEY=... PERPLEXITY_API_KEY=pplx-... \
  nu bin/fill-keys.nu          # or: sh bin/fill-keys.sh  /  ion bin/fill-keys.ion

nu bin/fill-keys.nu --yes        # auto-approve all files
nu bin/fill-keys.nu --help       # options (Nushell/POSIX; Ion: see header)
```

Each file is presented with a `[Y/n]` prompt so you control which hosts get their keys
filled. Omitted keys keep their placeholder, so that server simply stays inert. VS Code
is excluded from the list — it prompts for the Context7/Brave keys via its own `inputs`
mechanism.

## Project Posture

`mcp-servers` is a **personal hobby project** under the
[Spacecraft Software](https://SpacecraftSoftware.org/) umbrella. It is developed at hobby
pace and shaped around the maintainer's own toolchain, not a general audience.

- **No warranty, no liability.** See [`NOTICE.md`](./NOTICE.md).
- **Contributions are welcome but not guaranteed.** See [`CONTRIBUTING.md`](./CONTRIBUTING.md).
- **Forking is encouraged.** GPL-3.0-or-later is there for exactly that.

## License

Licensed under **GPL-3.0-or-later**. This repository is [REUSE](https://reuse.software)-
compliant: license texts live in [`LICENSES/`](./LICENSES) and per-file copyright/license
metadata is declared in [`REUSE.toml`](./REUSE.toml). The root `LICENSE` is retained for
GitHub's license detection.

## Maintainer

Mohamed Hammad &lt;Mohamed.Hammad@SpacecraftSoftware.org&gt;
Copyright (C) 2026 Mohamed Hammad &amp; Spacecraft Software
Website: <https://SpacecraftSoftware.org/>

---

*--- Forged in Spacecraft Software ---*
