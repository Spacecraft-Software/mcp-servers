#!/usr/bin/env sh
# SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
# SPDX-License-Identifier: GPL-3.0-or-later
#
# fill-keys.sh — POSIX (Bash / Brush / dash / ash) port of fill-keys.nu.
#
# Fills the MCP config files' placeholder API-key tokens with real values
# directly in their live config paths. Each file is presented with a Y/N
# prompt before modification, so the user controls which hosts get their
# keys filled. Supports unattended (--yes) mode for CI.
#
#   Env var          Fills placeholder           Server
#   CONTEXT7_API_KEY  YOUR_CONTEXT7_API_KEY       context7
#   BRAVE_API_KEY     YOUR_BRAVE_API_KEY          brave-search
#   PERPLEXITY_API_KEY YOUR_PERPLEXITY_API_KEY    perplexity

#
# Requires: sd (https://github.com/chmln/sd — `cargo install sd` or `nix run nixpkgs#sd`).

set -eu

usage() {
	cat <<'EOF'
Usage: fill-keys.sh [--yes]

  --yes       skip all Y/N prompts (auto-yes), for unattended/CI use
  -h, --help  show this help

Env vars: CONTEXT7_API_KEY, BRAVE_API_KEY, PERPLEXITY_API_KEY
EOF
}

yes_all=0
while [ $# -gt 0 ]; do
	case "$1" in
		--yes) yes_all=1; shift ;;
		-h|--help) usage; exit 0 ;;
		*) printf 'error: unknown argument: %s\n' "$1" >&2; usage >&2; exit 2 ;;
	esac
done

command -v sd >/dev/null 2>&1 || {
	printf 'error: sd not found. Install it: cargo install sd  (or: nix run nixpkgs#sd)\n' >&2
	exit 1
}

interactive=0
if [ "$yes_all" = 0 ] && [ -t 0 ] && [ -z "${CI:-}" ] && [ -z "${CLAUDECODE:-}" ]; then
	interactive=1
fi

# Prompt helper — reads from /dev/tty so the pipe-based file list below
# does not steal stdin.
prompt_yn() { # $1 = label
	[ -e /dev/tty ] || return 0
	while :; do
		printf '%s' "$1" >/dev/tty
		IFS= read -r _ans </dev/tty || _ans=
		case "$_ans" in
			[Yy]|'') return 0 ;;
			[Nn])    return 1 ;;
			*)       printf '  Please answer y or n.\n' >/dev/tty ;;
		esac
	done
}

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

tab=$(printf '\t')
resolved=$(mktemp)
trap 'rm -f "$resolved"' EXIT INT TERM

filled=0
filled_names=

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
	fi
done <<'EOF'
YOUR_CONTEXT7_API_KEY|CONTEXT7_API_KEY|1
YOUR_BRAVE_API_KEY|BRAVE_API_KEY|1
YOUR_PERPLEXITY_API_KEY|PERPLEXITY_API_KEY|1
EOF

# Live config paths: label|path
processed=0
skipped_files=0

while IFS='|' read -r label path; do
	[ -n "$label" ] || continue
	eval "path=$path"
	[ -f "$path" ] || { printf 'warning: %s not found at %s — skipping\n' "$label" "$path" >&2; continue; }
	if [ "$interactive" = 1 ]; then
		prompt_yn "Fill keys in $label ($path)? [Y/n] " || { skipped_files=$((skipped_files + 1)); continue; }
	fi
	while IFS="$tab" read -r ph val; do
		sd -s "$ph" "$val" "$path"
	done <"$resolved"
	processed=$((processed + 1))
done <<'EOF'
Antigravity (main)|${HOME}/.gemini/config/mcp_config.json
Antigravity (alt)|${HOME}/.gemini/antigravity/mcp_config.json
GitHub Copilot CLI|${HOME}/.copilot/mcp-config.json
Claude Code|${HOME}/.claude.json
OpenClaude|${HOME}/.openclaude.json
Codex CLI|${HOME}/.codex/config.toml
Grok|${HOME}/.grok/config.toml
Kimi Code|${HOME}/.kimi-code/config.toml
Gemini (settings)|${HOME}/.gemini/settings.json
Gemini (enablement)|${HOME}/.gemini/mcp-server-enablement.json
Qwen Code|${HOME}/.qwen/settings.json
OpenCode|${HOME}/.config/opencode/opencode.jsonc
Mimo Code|${HOME}/.config/mimocode/mimocode.jsonc
Goose|${HOME}/.config/goose/config.yaml
EOF

printf 'Filled %s of 3 placeholders in %s files' "$filled" "$processed"
[ "$skipped_files" -gt 0 ] && printf ' (%s declined)' "$skipped_files"
printf '\n'
[ -n "$filled_names" ] && printf 'Filled:%s\n' "$filled_names"
