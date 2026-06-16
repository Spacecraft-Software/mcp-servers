# mcp-servers

Per-tool **MCP (Model Context Protocol) server configurations** for the coding agents
and editors I use. Each directory holds the MCP config fragment for one host, in that
host's own dialect. They all wire up the same nine servers:

| Server | Transport | Endpoint / command | Auth |
|--------|-----------|--------------------|------|
| **nixos** | stdio | `nix run github:utensils/mcp-nixos --` ([mcp-nixos](https://github.com/utensils/mcp-nixos)) — nixpkgs / NixOS options | none |
| **context7** | http | `https://mcp.context7.com/mcp` — Upstash Context7 library docs | `CONTEXT7_API_KEY` |
| **microsoft-learn** | http | `https://learn.microsoft.com/api/mcp` — Microsoft Learn docs | none |
| **github** | http | `https://api.githubcopilot.com/mcp/` — GitHub API (repos, PRs, issues, code search) | GitHub PAT |
| **filesystem** | stdio | `npx -y @modelcontextprotocol/server-filesystem <path>` — sandboxed file access | none (set a path) |
| **fetch** | stdio | `npx -y @modelcontextprotocol/server-fetch` — fetch live web content | none |
| **memory** | stdio | `npx -y @modelcontextprotocol/server-memory` — knowledge-graph memory | none |
| **brave-search** | stdio | `npx -y @modelcontextprotocol/server-brave-search` — web search | `BRAVE_API_KEY` |
| **sequential-thinking** | stdio | `npx -y @modelcontextprotocol/server-sequential-thinking` — step-by-step reasoning | none |

The `npx`-based servers need Node.js 18+. `github`, `brave-search`, and `filesystem`
need a token or path filled in before they work (see Notes).

## Supported hosts

| Directory | Host | Live config path |
|-----------|------|------------------|
| `Antigravity/` | Antigravity | editor-managed |
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
| `Goose/` | goose | `~/.config/goose/config.yaml` |

## Notes

- Files are **templates** with placeholders — replace these locally, never commit real
  values: `YOUR_CONTEXT7_API_KEY`, `YOUR_GITHUB_PAT`, `YOUR_BRAVE_API_KEY`, and the
  `filesystem` server's `/path/to/your/workspace`. Until they're filled in, those servers
  won't connect (the other servers work as-is). VS Code instead prompts for the Context7
  and Brave keys via its `inputs` mechanism, and uses built-in Copilot auth for `github`.
- Schemas differ per host (e.g. Qwen uses `httpUrl`, Codex uses `http_headers`, Copilot
  CLI omits the `mcpServers` wrapper). See `CLAUDE.md` for the full per-host schema table.

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
