# mcp-servers

Per-tool **MCP (Model Context Protocol) server configurations** for the coding agents
and editors I use. Each directory holds the MCP config fragment for one host, in that
host's own dialect. They all wire up the same three servers:

- **nixos** — [`mcp-nixos`](https://github.com/utensils/mcp-nixos), queries nixpkgs / NixOS options (stdio, `nix run github:utensils/mcp-nixos --`).
- **context7** — Upstash Context7 docs server (`https://mcp.context7.com/mcp`, needs `CONTEXT7_API_KEY`).
- **microsoft-learn** — Microsoft Learn docs server (`https://learn.microsoft.com/api/mcp`, no auth).

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

- Files are **templates** — the Context7 key is the `YOUR_CONTEXT7_API_KEY` placeholder;
  replace it locally (never commit a real key).
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
