# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this repo is

A personal collection of **MCP (Model Context Protocol) server configurations**, one config file per host application. The deliverable is the config files themselves, but they are no longer written by hand: `mcp.toml` is the source of truth and `mcpctl/` (Rust) generates the templates, checks them, and deploys them to the live configs under `$HOME`. It is a Spacecraft Software-umbrella project (Personal posture, `GPL-3.0-or-later`); its GitHub remote is `Spacecraft-Software/mcp-servers` (migrated from `UnbreakableMJ/mcp-servers`).

Validation is that each config is well-formed and matches its host's schema, that every host declares the same servers (`mcpctl check`), that the templates still match the manifest (`mcpctl render --check`), and that `reuse lint` stays clean.

## Common Development Tasks

Everything goes through `mcpctl`, a Rust binary in `mcpctl/`. **`mcp.toml` at the repo
root is the single source of truth**; every file under a host directory is generated from
it and must never be hand-edited.

The toolchain: `nix develop` (the `rustup` toolchains on this machine are linked against
`/lib64/ld-linux-x86-64.so.2` and cannot execute on NixOS), then
`cargo build --release --manifest-path mcpctl/Cargo.toml`.

| Command | What it does |
|---------|--------------|
| `mcpctl check` | Every template parses, every host declares the same twelve servers, and no host disagrees about how to invoke one. Replaces the old `.github/validate-configs.py`. |
| `mcpctl render` | Regenerates all 13 templates plus the two companion files from `mcp.toml`. |
| `mcpctl render --check` | Fails instead of writing. CI gate against hand-edited templates. |
| `mcpctl deploy` | Pushes the manifest into the ~17 live configs under `$HOME`. |
| `mcpctl fill-keys` | Writes real API keys into the live configs. |

`--json` on any command gives a machine-readable envelope. `cargo test` covers dialect
round-trips, the documented host quirks, and the in-place splice.

### Changing a server

1. Edit `mcp.toml` — never a file under a host directory.
2. `mcpctl render` — updates all 13 templates.
3. `mcpctl deploy` — propagates to the live configs. **This step is the point of the
   tool.** Nothing reads the templates; they are a reference, not a source. Historically
   a change that stopped at step 2 reached no host while `git status` stayed clean —
   that is how the `fetch` `mcp<2` pin (`cc1fcfb`) shipped correct in all 13 templates
   and broken on every host, and how `perplexity` sat in every template and only 7 of 15
   live configs.
4. Restart every host. A failed MCP server is not retried in-process.

A deliberate per-host difference goes in `[servers.overrides.<Host>]` with a comment
saying why. Anything not recorded there is drift and `mcpctl check` fails on it.

### Adding a new host

Two edits, because the split matters:

- **`mcpctl/src/dialect.rs`** — one row in `HOSTS`, describing the host's *mechanics*:
  which key holds a URL, whether env vars go under `env` / `environment` / `envs`,
  whether the command is split or joined, indentation, the `type` values, how it spells
  "enabled". These are facts about the host's schema, so they live in code.
- **`mcp.toml`** — one `[[hosts]]` entry giving the template path, the live config
  path(s), and any flags (`header`, `schema_url`, `emit_inputs`, `prune_strays`,
  `guard_process`, companion files). This is content, so it lives in the manifest.

Then `mcpctl render`. The manifest validates that the two sides agree: a host in one and
not the other is an error, not a silent omission.

### Deploying safely

`deploy` writes files the user and their tools own, so it holds these properties. Do not
weaken them:

- **Only the MCP block moves.** It is located by byte range and spliced; `~/.claude.json`
  is 238 KB / 6,835 lines of which ~60 change. TOML goes through `toml_edit`, which
  preserves untouched spans natively.
- **Unmanaged servers are preserved byte-for-byte.** goose keeps ~16 builtin/platform
  extensions in the same `extensions` block (`prune_strays = false` for that reason).
  Elsewhere a stray may be pruned, but only after an explicit `y`.
- **Secrets.** A real key already in the live file is carried into the rewritten block. A
  secret with no available value is **omitted, not written as a placeholder** — a literal
  `YOUR_CONTEXT7_API_KEY` is sent as a credential and rejected, whereas an absent header
  leaves Context7 working anonymously. Placeholders belong in templates, which are
  documentation.
- **Empty collections are preserved.** The emitters must write `available_tools: []` and
  `settings: {}` rather than skipping them; skipping silently stripped keys from 13 goose
  extensions.
- **Never corrupt.** The result is re-parsed (managed servers only — builtins legitimately
  have neither `cmd` nor `uri`) before writing, the write is a rename over a fully written
  temporary, and the previous contents go to `~/.mcp-backup/<ISO8601>/`.
- **Running-host guard.** Claude Code rewrites `~/.claude.json` on exit, so `deploy`
  refuses that host while `claude` is running. `--force` overrides.
- **Dry-run by default** with no TTY, or when `CI` / `CLAUDECODE` is set. An agent-driven
  run reports; it does not write.

`deploy` and `fill-keys` build server entries through the same `render::server_entry` +
`deploy::resolve_secrets` path. That is load-bearing: an earlier `fill-keys` patched the
live object directly, which appended `env` *after* `disabled` while `deploy` emits it
before, so the two commands reordered each other's output forever.

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
`<host>.jsonc`, `<host>.json`, `config.json`) and where `mcpctl` reads and writes.

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
- **context7** (Upstash) — HTTP, `https://mcp.context7.com/mcp`, needs a `CONTEXT7_API_KEY`. Sent as `Authorization: Bearer <key>` on every host (VS Code supplies it from a prompted `input`; everywhere else the placeholder `YOUR_CONTEXT7_API_KEY` is filled in). The older `CONTEXT7_API_KEY:` header still authenticates — verified against the live endpoint, where a bad key under it is rejected rather than ignored — but it is undocumented upstream, and because unauthenticated requests also succeed at a lower rate limit, a header that stopped being read would be indistinguishable from one that works.
- **microsoft-learn** — HTTP, `https://learn.microsoft.com/api/mcp`, no auth.
- **crates** — stdio, `crates-mcp` ([crates-mcp](https://crates.io/crates/crates-mcp) via `cargo install`), queries Rust crates from crates.io and docs.rs.
- **bravais-cli** — stdio, `bravais-cli mcp`, queries tool preferences and rewrites shell commands.

**The seven generic servers** (templates-only, placeholders):
- **filesystem** — stdio, `npx -y @modelcontextprotocol/server-filesystem <path>`. Hardcoded path `/spacecraft-software` across all hosts.
- **fetch**, **engram**, **sequential-thinking** — stdio, no auth. `fetch` is `uvx --with 'mcp<2' mcp-server-fetch`; `sequential-thinking` is `npx -y @modelcontextprotocol/server-sequential-thinking`; `engram` is `engram --db ~/.gemini/engram.db mcp`. **The `mcp<2` pin on fetch is load-bearing**: `mcp-server-fetch` 2026.7.10 (latest) declares only `mcp>=1.1.3` but imports `McpError`, which mcp 2.0.0 renamed to `MCPError` — unpinned, the server dies at import with `ImportError: cannot import name 'McpError'` and the host reports `calling "initialize": EOF`. Drop the pin only once upstream ships a release that imports `MCPError`.
- **brave-search** — stdio, `npx -y @brave/brave-search-mcp-server` ([brave-search-mcp-server](https://github.com/brave/brave-search-mcp-server)), env `BRAVE_API_KEY=YOUR_BRAVE_API_KEY`.
- **perplexity** — stdio, `npx -y perplexity-mcp`, env `PERPLEXITY_API_KEY=YOUR_PERPLEXITY_API_KEY`.
- **terminal** — stdio, `npx -y mcp-server-terminal` ([mcp-server-terminal](https://github.com/aybelatchane/mcp-server-terminal)), env `RUST_LOG=error`, `NO_COLOR=1`, `TERM=dumb` (keeps TUI output plain for the agent); no auth.

## Conventions

- Antigravity's file uses **2-space** indentation; VS Code's uses **tabs**. This is encoded as `indent` on each `Host`, so the renderer preserves it for you. Host directory names are **case-sensitive and canonical** (`Antigravity/`, `VSCode/`) — do not reintroduce lowercase `antigravity/` or `.vscode/` variants (a past PR did; they were consolidated).
- Never commit a real secret — templates carry placeholders (`YOUR_CONTEXT7_API_KEY`, `YOUR_BRAVE_API_KEY`, `YOUR_PERPLEXITY_API_KEY`). The filesystem server uses the hardcoded `/spacecraft-software` path. Servers needing placeholders stay inert until filled in locally.
- Templates are **generated from `mcp.toml`** and are the canonical superset. Never edit one by hand; change the manifest and run `mcpctl render`, then `mcpctl deploy` to reach the hosts.

## Tooling

`mcpctl/` is a Rust binary — the repo's only build target. `nix develop` provides the
toolchain; `flake.nix` also exposes it as a package. `Cargo.lock` is tracked.

It superseded `.github/validate-configs.py` (which only checked that files parsed, not
that they agreed) and `bin/fill-keys.{nu,sh,ion}`.

The three shell ports are **retained as legacy**, for machines without a Rust toolchain,
and carry a banner saying so. Keep their path lists in sync with `mcp.toml` — they had
already drifted, missing `~/.gemini/antigravity-ide/mcp_config.json` entirely. They are
plain `sd` substitution and can only fill a placeholder that is already present, so they
are of little use after a `deploy` (which never writes one).

Two traps if you edit them, both found by running the ports rather than reading them:
`sd '|' '\n'` treats `|` as a regex alternation matching at every position — the Ion port
did this and silently skipped all 15 files while printing "Filled 3 of 3 placeholders" —
and adding `-s` fixes the pattern but then emits a literal backslash-n as the replacement.
The Ion port now iterates paths directly and splits nothing.

## Code architecture

```
mcp.toml                 content: which servers exist, where each host's files live
mcpctl/src/dialect.rs    mechanics: how each host spells the same concepts
        |
        +-- render  ---> 13 tracked templates (generated, never hand-edited)
        |                        |
        +-- deploy  ------------ + ---> ~17 live configs under $HOME
        +-- fill-keys ------------------^  (real keys, never in the repo)
        +-- check   ---> parity across templates (CI gate)
```

The content/mechanics split is the organizing idea. A server change touches only
`mcp.toml`; a newly supported host touches only `HOSTS` plus one `[[hosts]]` entry.
Neither requires understanding the other.

### Modules

| Module | Responsibility |
|--------|----------------|
| `manifest.rs` | Parses and validates `mcp.toml`; applies per-host overrides |
| `dialect.rs` | The `HOSTS` table; parses any host file into a normalized `Transport` |
| `emit.rs` | JSON / TOML / YAML writers tuned to look hand-written |
| `render.rs` | Manifest into templates |
| `splice.rs` | Locating and replacing just the MCP block of a live file |
| `deploy.rs` | Merge, secret resolution, backup, atomic write |
| `fill_keys.rs` | Real keys into live configs |
| `check.rs` | Cross-host parity |

The 13 hosts do **not** get 13 renderers. They differ along ~9 axes captured as fields on
`Host`, interpreted by three serializers. Every trap in the schema table below is one
field: Qwen's `httpUrl` is `url_field`, Codex's `http_headers` is `headers_field`,
Copilot's missing wrapper is `wrapper: None`, opencode's `environment` is `env_field`.

Comparison in `check` runs on normalized `Transport` values, never on raw keys. A naive
key-level comparison reports opencode as "missing `env`" when it uses `environment` — a
false positive an early throwaway script actually produced.

### Constraints

1. **No secrets in git.** Templates carry placeholders; real keys only ever reach `$HOME`.
2. **Templates are generated.** Hand-editing one is reverted by the next `render`, and CI
   fails on the difference.
3. **Live configs belong to their tools.** See "Deploying safely" above.
4. **All hosts declare the same twelve servers**, modulo recorded overrides.
5. **REUSE compliance** — every file `GPL-3.0-or-later` via `REUSE.toml`.
6. **Signed commits** (§6.3).

## Development workflow

```sh
nix develop
cargo test  --manifest-path mcpctl/Cargo.toml
cargo clippy --manifest-path mcpctl/Cargo.toml --all-targets -- -D warnings
cargo build --release --manifest-path mcpctl/Cargo.toml

./mcpctl/target/release/mcpctl check
./mcpctl/target/release/mcpctl render --check
reuse lint
```

Before changing `deploy` or `fill_keys`, exercise them against a copy of the real configs
rather than reasoning about the code:

```sh
mkdir -p /tmp/homesim && cp -r ~/.codex ~/.gemini ~/.config/goose /tmp/homesim/   # etc.
HOME=/tmp/homesim ./mcpctl/target/release/mcpctl deploy --yes --force
```

Both bugs that mattered — 13 goose extensions silently losing `available_tools: []`, and
placeholders being written into live configs as credentials — were found this way and
would not have been found by reading the diff.

### Common pitfalls

1. **Schema mismatches**: Qwen uses `httpUrl`; Codex uses `http_headers`; Copilot CLI has
   no wrapper; opencode uses `environment` and a joined command array; goose uses
   `cmd`/`envs`/`uri`. All are pinned by `documented_host_quirks_survive`.
2. **Editing a template instead of `mcp.toml`** — silently reverted.
3. **Skipping `deploy`** — the original failure mode this tool exists to prevent.
4. **Dropping an empty collection** when emitting; it is someone else's data.
5. **Ordering.** `serde_json` and `toml` are both built with `preserve_order`; without it
   env blocks alphabetize themselves. Use `shift_remove`, never `remove`, on these maps.