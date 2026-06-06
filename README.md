# mcp-servers

A collection of useful [Model Context Protocol (MCP)](https://modelcontextprotocol.io/) server configurations for [VSCode](https://code.visualstudio.com/) and [Antigravity](https://antigravity.google/).

## Included MCP Servers

| Server | Package | Description |
|--------|---------|-------------|
| **GitHub** | Remote: `https://api.githubcopilot.com/mcp/` | Full GitHub API access — repos, PRs, issues, code search, and more |
| **Filesystem** | `@modelcontextprotocol/server-filesystem` | Sandboxed read/write access to local project files |
| **Fetch** | `@modelcontextprotocol/server-fetch` | Retrieve live web content and APIs for real-time context |
| **Memory** | `@modelcontextprotocol/server-memory` | Persistent knowledge-graph memory across sessions |
| **Brave Search** | `@modelcontextprotocol/server-brave-search` | Privacy-focused real-time web search (requires a [free Brave API key](https://brave.com/search/api/)) |
| **Sequential Thinking** | `@modelcontextprotocol/server-sequential-thinking` | Structured step-by-step reasoning for complex tasks |

## Prerequisites

- **Node.js** v18 or later (for `npx`-based servers)
- **GitHub Personal Access Token** (for the GitHub MCP server outside of VSCode's native Copilot auth)
- **Brave Search API Key** (optional — only needed for the `brave-search` server; free tier available at https://brave.com/search/api/)

## VSCode Setup

The `.vscode/mcp.json` file is automatically picked up by VSCode when you open this workspace. All servers will be available in **Copilot Agent Mode** via the tools icon in the Copilot Chat panel.

1. Open this repository in VSCode.
2. Open the Copilot Chat panel and switch to **Agent** mode.
3. Click the **🔧 tools** icon — all MCP servers should appear in the list.
4. When first using `brave-search`, VSCode will prompt you for your Brave API key.

> **Note:** The `github` server uses VSCode's existing GitHub Copilot authentication automatically — no PAT needed.

For more details see the [VSCode MCP documentation](https://code.visualstudio.com/docs/copilot/customization/mcp-servers).

## Antigravity Setup

Copy the provided reference configuration to your Antigravity user config directory.

### macOS / Linux

```bash
mkdir -p ~/.gemini/antigravity
cp antigravity/mcp_config.json ~/.gemini/antigravity/mcp_config.json
```

### Windows

```powershell
New-Item -ItemType Directory -Force -Path "$env:USERPROFILE\.gemini\antigravity"
Copy-Item antigravity\mcp_config.json "$env:USERPROFILE\.gemini\antigravity\mcp_config.json"
```

Then set the required environment variables before launching Antigravity:

```bash
export GITHUB_PERSONAL_ACCESS_TOKEN=ghp_your_token_here  # GitHub PAT
export BRAVE_API_KEY=your_brave_api_key_here              # Optional
```

Restart Antigravity and open **... → MCP Servers → Manage MCP Servers** to verify the servers are listed.

For more details see the [Antigravity MCP documentation](https://antigravity.google/docs/mcp) and the [GitHub MCP server Antigravity guide](https://github.com/github/github-mcp-server/blob/main/docs/installation-guides/install-antigravity.md).

## Configuration Files

| File | Purpose |
|------|---------|
| `.vscode/mcp.json` | VSCode workspace-level MCP configuration (loaded automatically) |
| `antigravity/mcp_config.json` | Reference configuration for Antigravity (copy to `~/.gemini/antigravity/`) |

## Notes

- **Filesystem path (Antigravity):** The `filesystem` server in `antigravity/mcp_config.json` uses a placeholder path (`/path/to/your/workspace`). Replace it with the absolute path of the directory you want the AI to access before copying the file.
- **GitHub endpoint:** Both configs target `https://api.githubcopilot.com/mcp/` — the official remote GitHub MCP server. VSCode authenticates via its built-in Copilot session; Antigravity uses the `GITHUB_PERSONAL_ACCESS_TOKEN` environment variable.
