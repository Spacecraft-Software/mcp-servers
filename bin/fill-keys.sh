#!/usr/bin/env sh
# SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
# SPDX-License-Identifier: GPL-3.0-or-later
#
# fill-keys.sh — POSIX (Bash / Brush / dash / ash) port of fill-keys.nu.
#
# Fills the MCP config templates' placeholders with real values and writes ready-to-use
# copies into a gitignored output dir (default: dist/). Values come from env vars; any
# unset ones are prompted for (hidden input) when interactive, or left as placeholders
# when not (CI/agent). The tracked templates are never modified.
#
#   Env var          Fills placeholder           Server
#   CONTEXT7_API_KEY  YOUR_CONTEXT7_API_KEY       context7
#   BRAVE_API_KEY     YOUR_BRAVE_API_KEY          brave-search
#   GITHUB_PAT        YOUR_GITHUB_PAT             github
#   WORKSPACE_PATH    /path/to/your/workspace     filesystem

#
# Requires: sd (https://github.com/chmln/sd — `cargo install sd` or `nix run nixpkgs#sd`).

set -eu

usage() {
	cat <<'EOF'
Usage: fill-keys.sh [--out DIR]

  --out DIR   output directory (gitignored), relative to the repo root [default: dist]
  -h, --help  show this help

Env vars: CONTEXT7_API_KEY, BRAVE_API_KEY, GITHUB_PAT, WORKSPACE_PATH
EOF
}

out="dist"
while [ $# -gt 0 ]; do
	case "$1" in
		--out) out=${2:?--out needs a value}; shift 2 ;;
		--out=*) out=${1#--out=}; shift ;;
		-h|--help) usage; exit 0 ;;
		*) printf 'error: unknown argument: %s\n' "$1" >&2; usage >&2; exit 2 ;;
	esac
done

command -v sd >/dev/null 2>&1 || {
	printf 'error: sd not found. Install it: cargo install sd  (or: nix run nixpkgs#sd)\n' >&2
	exit 1
}

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo=$(dirname -- "$script_dir")
case "$out" in
	/*) out_root=$out ;;
	*)  out_root=$repo/$out ;;
esac

interactive=0
if [ -t 0 ] && [ -z "${CI:-}" ] && [ -z "${CLAUDECODE:-}" ]; then
	interactive=1
fi

# Prompt helpers read from and write to the terminal directly, so the token/host
# here-docs below (which occupy stdin) don't swallow the typed answer.
read_value() { # $1 = prompt, $2 = 1 for hidden
	[ -e /dev/tty ] || return 1
	if [ "$2" = 1 ]; then
		_old=$(stty -g </dev/tty 2>/dev/null) || _old=
		[ -n "$_old" ] && stty -echo </dev/tty 2>/dev/null
	fi
	printf '%s' "$1" >/dev/tty
	IFS= read -r _val </dev/tty || _val=
	if [ "$2" = 1 ]; then
		[ -n "${_old:-}" ] && stty "$_old" </dev/tty 2>/dev/null
		printf '\n' >/dev/tty
	fi
	printf '%s' "$_val"
}

resolved=$(mktemp)
trap 'rm -f "$resolved"' EXIT INT TERM

filled=0
filled_names=
skipped_names=
tab=$(printf '\t')

while IFS='|' read -r placeholder envvar secret; do
	[ -n "$placeholder" ] || continue
	eval "value=\${$envvar:-}"
	if [ -z "$value" ] && [ "$interactive" = 1 ]; then
		value=$(read_value "$envvar [blank to skip]: " "$secret") || value=
	fi
	if [ -n "$value" ]; then
		printf '%s%s%s\n' "$placeholder" "$tab" "$value" >>"$resolved"
		filled=$((filled + 1))
		filled_names="$filled_names $envvar"
	else
		skipped_names="$skipped_names $envvar"
	fi
done <<'EOF'
YOUR_CONTEXT7_API_KEY|CONTEXT7_API_KEY|1
YOUR_BRAVE_API_KEY|BRAVE_API_KEY|1
YOUR_GITHUB_PAT|GITHUB_PAT|1
/path/to/your/workspace|WORKSPACE_PATH|0
EOF

written=0
while IFS='|' read -r src live; do
	[ -n "$src" ] || continue
	[ -f "$repo/$src" ] || { printf 'error: missing template %s\n' "$src" >&2; exit 1; }
	dst=$out_root/$src
	mkdir -p "$(dirname -- "$dst")"
	cp -- "$repo/$src" "$dst"
	while IFS="$tab" read -r ph val; do
		sd -s "$ph" "$val" "$dst"
	done <"$resolved"
	written=$((written + 1))
done <<'EOF'
Antigravity/mcp_config.json|(Antigravity — editor-managed)
VSCode/mcp.json|<workspace>/.vscode/mcp.json
GitHubCopilotCLI/mcp-config.json|~/.copilot/mcp-config.json
ClaudeCode/.mcp.json|~/.claude.json (mcpServers)
OpenClaude/.mcp.json|~/.openclaude.json (mcpServers)
Codex/config.toml|~/.codex/config.toml
Grok/config.toml|~/.grok/config.toml
Kimi/config.toml|~/.kimi-code/config.toml
Gemini/settings.json|~/.gemini/settings.json
Gemini/mcp-server-enablement.json|~/.gemini/mcp-server-enablement.json
Qwen/settings.json|~/.qwen/settings.json
OpenCode/opencode.jsonc|~/.config/opencode/opencode.jsonc
Mimo/mimocode.jsonc|~/.config/mimocode/mimocode.jsonc
Goose/config.yaml|~/.config/goose/config.yaml
EOF

printf 'Filled %s of 4 placeholders into %s/ (%s files)\n' "$filled" "$out_root" "$written"
[ -n "$filled_names" ] && printf 'Filled:%s\n' "$filled_names"
[ -n "$skipped_names" ] && printf 'Left as placeholders (those servers stay inert):%s\n' "$skipped_names"
printf '\nInstall destinations:\n'
while IFS='|' read -r src live; do
	[ -n "$src" ] || continue
	printf '  %-34s -> %s\n' "$src" "$live"
done <<'EOF'
Antigravity/mcp_config.json|(Antigravity — editor-managed)
VSCode/mcp.json|<workspace>/.vscode/mcp.json
GitHubCopilotCLI/mcp-config.json|~/.copilot/mcp-config.json
ClaudeCode/.mcp.json|~/.claude.json (mcpServers)
OpenClaude/.mcp.json|~/.openclaude.json (mcpServers)
Codex/config.toml|~/.codex/config.toml
Grok/config.toml|~/.grok/config.toml
Kimi/config.toml|~/.kimi-code/config.toml
Gemini/settings.json|~/.gemini/settings.json
Gemini/mcp-server-enablement.json|~/.gemini/mcp-server-enablement.json
Qwen/settings.json|~/.qwen/settings.json
OpenCode/opencode.jsonc|~/.config/opencode/opencode.jsonc
Mimo/mimocode.jsonc|~/.config/mimocode/mimocode.jsonc
Goose/config.yaml|~/.config/goose/config.yaml
EOF
