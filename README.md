# mcp-servers

Per-tool **MCP (Model Context Protocol) server configurations** for the coding agents
and editors I use. Each directory holds the MCP config fragment for one host, in that
host's own dialect. They all wire up the same twelve servers:

| Server | Transport | Endpoint / command | Auth |
|--------|-----------|--------------------|------|
| **nixos** | stdio | `nix run github:utensils/mcp-nixos --` ([mcp-nixos](https://github.com/utensils/mcp-nixos)) — nixpkgs / NixOS options | none |
| **context7** | http | `https://mcp.context7.com/mcp` — Upstash Context7 library docs | `CONTEXT7_API_KEY`, sent as `Authorization: Bearer` |
| **microsoft-learn** | http | `https://learn.microsoft.com/api/mcp` — Microsoft Learn docs | none |
| **bravais-cli** | stdio | `bravais-cli mcp` — Bravais OS command replacement and shell translator | none |
| **filesystem** | stdio | `npx -y @modelcontextprotocol/server-filesystem <path>` — sandboxed file access | none (set a path) |
| **fetch** | stdio | `uvx --with 'mcp<2' mcp-server-fetch` — fetch live web content (the `mcp<2` pin is required: mcp-server-fetch 2026.7.10 imports `McpError`, renamed to `MCPError` in mcp 2.0) | none |
| **engram** | stdio | `engram --db ~/.gemini/engram.db mcp` — shared verbatim chat memory | none |
| **brave-search** | stdio | `npx -y @brave/brave-search-mcp-server` — web, local, news, image, video search | `BRAVE_API_KEY` |
| **perplexity** | stdio | `npx -y perplexity-mcp` — Perplexity search | `PERPLEXITY_API_KEY` |
| **sequential-thinking** | stdio | `npx -y @modelcontextprotocol/server-sequential-thinking` — step-by-step reasoning | none |
| **crates** | stdio | `crates-mcp` ([crates-mcp](https://crates.io/crates/crates-mcp) via `cargo install`) — Rust crate search and docs | none |
| **terminal** | stdio | `npx -y mcp-server-terminal` ([mcp-server-terminal](https://github.com/aybelatchane/mcp-server-terminal)) — TUI/CLI terminal automation | none |

The `npx`-based servers need Node.js 18+. `brave-search`, `perplexity`, and `filesystem`
need a token or path filled in before they work (see Notes).

## Supported hosts

| Directory | Host | Live config path |
|-----------|------|------------------|
| `Antigravity/` | Antigravity | `~/.gemini/config/mcp_config.json` (CLI)<br>`~/.gemini/antigravity/mcp_config.json` (2.0)<br>`~/.gemini/antigravity-ide/mcp_config.json` (IDE) |
| `VSCode/` | VS Code | `.vscode/mcp.json` |
| `GitHubCopilotCLI/` | GitHub Copilot CLI | `~/.copilot/mcp-config.json` |
| `ClaudeCode/` | Claude Code | `~/.claude.json` (+ `~/.claude/settings.json`) |
| `OpenClaude/` | OpenClaude | `~/.openclaude.json` |
| `Codex/` | OpenAI Codex | `~/.codex/config.toml` |
| `Grok/` | Grok CLI | `~/.grok/config.toml` |
| `Kimi/` | Kimi Code CLI | `~/.kimi-code/mcp.json` |
| `Gemini/` | Gemini CLI | `~/.gemini/settings.json` |
| `Qwen/` | Qwen Code | `~/.qwen/settings.json` |
| `OpenCode/` | opencode | `~/.config/opencode/opencode.jsonc` |
| `Mimo/` | Mimo Code | `~/.config/mimocode/mimocode.jsonc` |
| `Goose/` | goose | `~/.config/goose/config.yaml` |

## Using it

Everything goes through `mcpctl`. `mcp.toml` at the repository root is the single source
of truth; the host directories are **generated** from it.

```sh
nix develop                    # cargo toolchain (see Notes)
cargo build --release --manifest-path mcpctl/Cargo.toml
alias mcpctl=./mcpctl/target/release/mcpctl

mcpctl check                   # every template parses and every host agrees
mcpctl render                  # regenerate the 13 templates from mcp.toml
mcpctl render --check          # fail instead of writing (this is what CI runs)
mcpctl deploy --dry-run        # what would change in your live configs under $HOME
mcpctl deploy --yes            # apply it
mcpctl fill-keys               # put real API keys into the live configs
```

Two commands answer "what can this do" without needing a repository, which is where an
agent should start:

```sh
mcpctl describe --json   # how this invocation resolved: format, color, interactivity
mcpctl schema --json     # every command as JSON Schema Draft 2020-12
mcpctl schema --format openai   # also: anthropic, gemini, mcp
```

`--json` on any command gives a `{metadata, data}` envelope. Errors give
`{metadata, error{code, exit_code, message, hint}}`, where `hint` is a **runnable
command** rather than a sentence about one:

```json
{"error":{"code":"NOT_FOUND","exit_code":3,
          "message":"`/nonexistent` is not readable: No such file or directory",
          "hint":"mcpctl check --repo ."}}
```

Exit codes are stable: `0` ok, `1` bad repository or config state, `2` bad invocation,
`3` not found, `4` refused for safety.

JSON is selected automatically when `AI_AGENT`, `AGENT`, or a truthy `CI` is present —
detected by **presence**, since real harnesses export descriptive values rather than `1`.
Under an agent the output is also compact rather than pretty-printed, and color is off.
`CLAUDECODE`, `CURSOR_AGENT`, and `GEMINI_CLI` are reported in `metadata.invoking_agent`
but never change behavior on their own.

**Changing a server is now one edit.** Edit `mcp.toml`, run `mcpctl render` to update all
13 templates, then `mcpctl deploy` to push it to the hosts, then restart the hosts. The
second step is the one that used to be missed: nothing reads the templates, so a change
that stops at `render` reaches no host while `git status` stays clean.

### Deploying to live configs

`deploy` edits files your tools own, so it is deliberately conservative:

- **Only the MCP block is replaced**, located by byte range. Deploying into a 238 KB
  `~/.claude.json` changes ~60 lines and leaves the other 6,800 byte-identical.
- **Servers it does not manage are never dropped.** goose keeps ~16 builtin extensions in
  the same block; they are reported and preserved. Elsewhere a stray can be pruned, but
  only after an explicit `y`.
- **A working key is never replaced by a placeholder**, and a placeholder is never written
  into a live config — an unfilled secret is omitted and reported instead, because a
  rejected credential is worse than an absent one.
- **Every rewrite is re-parsed before it is written**, the write is a rename over a fully
  written temporary, and the previous contents are copied to `~/.mcp-backup/<timestamp>/`.
- **It refuses to write to a config whose host is running** (Claude Code rewrites
  `~/.claude.json` on exit, silently reverting a deploy). Exit the host, or pass `--force`.
- **It defaults to `--dry-run`** whenever there is no terminal, or `CI` / `CLAUDECODE` is
  set, so an automated run reports rather than writes.

## Notes

- Files are **templates** with placeholders — replace these locally, never commit real
  values: `YOUR_CONTEXT7_API_KEY`, `YOUR_BRAVE_API_KEY`, `YOUR_PERPLEXITY_API_KEY`. The
  `filesystem` server uses the hardcoded path `/spacecraft-software`. Until placeholders
  are filled in, those servers won't connect (the other servers work as-is). VS Code
  instead prompts for the Context7, Brave, and Perplexity keys via its `inputs`
  mechanism.
- Schemas differ per host (e.g. Qwen uses `httpUrl`, Codex uses `http_headers`, Copilot
  CLI omits the `mcpServers` wrapper). See `CLAUDE.md` for the full per-host schema table.
- `sequential-thinking` is declared but disabled by default for **Claude Code only** —
  `.mcp.json` has no per-server disable field, so `ClaudeCode/settings.json` turns it off
  via `disabledMcpjsonServers`. Every other host runs it enabled. This is expressed once
  in `mcp.toml` as a per-host `enabled = false` override.
- **Do not hand-edit a file under a host directory.** They are generated; the next
  `mcpctl render` overwrites them, and CI fails when they disagree with `mcp.toml`.
- The build needs a working cargo. `nix develop` provides one; a `rustup` toolchain works
  on most systems but not on NixOS, where the downloaded binaries are linked against an
  ELF interpreter that does not exist there.

## Filling in your keys

`mcpctl fill-keys` writes your real values **into the live config files** under `$HOME`.
The tracked templates are never touched, so no secret can be committed.

| Env var | Server | Where it lands |
|---------|--------|----------------|
| `CONTEXT7_API_KEY` | context7 | a request header |
| `BRAVE_API_KEY` | brave-search | an environment variable |
| `PERPLEXITY_API_KEY` | perplexity | an environment variable |

```sh
mcpctl fill-keys                 # prompts for each key, with echo off

CONTEXT7_API_KEY=ctx7sk-... BRAVE_API_KEY=... PERPLEXITY_API_KEY=pplx-... \
  mcpctl fill-keys --yes         # unattended; any key already exported is used
```

A key given on the command line **overrides** whatever is already in the live config, so
this is also how you rotate one. A key you skip (blank answer, or env var unset) leaves
the live configs exactly as they are.

It reads `mcp.toml` to learn which server wants which key under which host-specific field
name, so it can *insert* a key into a host whose config never mentioned it. The previous
shell scripts substituted the literal string `YOUR_CONTEXT7_API_KEY` and could only fill a
placeholder that was already sitting in the file.

VS Code is handled but never receives a literal key: it resolves secrets through its own
`inputs` prompt, and `mcpctl` generates that `inputs` array from `mcp.toml`.

### Legacy shell ports

`bin/fill-keys.{sh,nu,ion}` are kept for machines without a Rust toolchain. They are plain
string substitution — they replace the placeholder tokens *wherever those tokens already
appear* in a live config, using [`sd`](https://github.com/chmln/sd) (the `.nu` port uses
native string operations).

That limits them: `mcpctl deploy` never writes a placeholder into a live config, so after a
deploy there is generally nothing for them to substitute. Use them only on a config you
populated by copying a template verbatim. `mcpctl fill-keys` is the supported path.

## Files for agents

| File | Reader |
|---|---|
| `AGENTS.md` | Generic agents — repository invariants, build/test commands, what not to edit |
| `CLAUDE.md` | Claude Code — the same, plus the reasoning behind each safety property |
| `SKILL.md` | Skill loaders — `mcpctl`'s capability surface |
| `CONTRIBUTING.md` | Human contributors |

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
