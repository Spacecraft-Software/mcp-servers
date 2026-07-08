#!/usr/bin/env nu
# SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
# SPDX-License-Identifier: GPL-3.0-or-later
#
# fill-keys.nu — fill the MCP config files' placeholder API-key tokens with
# real values directly in their live config paths. Each file is presented with
# a Y/N prompt before modification, so the user controls which hosts get their
# keys filled. Supports unattended (--yes) mode for CI.
#
#   Env var          Fills placeholder           Server
#   CONTEXT7_API_KEY  YOUR_CONTEXT7_API_KEY       context7
#   BRAVE_API_KEY     YOUR_BRAVE_API_KEY          brave-search
#   PERPLEXITY_API_KEY YOUR_PERPLEXITY_API_KEY    perplexity

#
# Usage:
#   CONTEXT7_API_KEY=ctx7sk-... nu bin/fill-keys.nu
#   nu bin/fill-keys.nu --yes
#   nu bin/fill-keys.nu --help

# Each placeholder, the env var that fills it, and whether it is a secret (hidden prompt).
const TOKENS = [
    [placeholder                  env_var            secret];
    ["YOUR_CONTEXT7_API_KEY"      "CONTEXT7_API_KEY"  true ]
    ["YOUR_BRAVE_API_KEY"         "BRAVE_API_KEY"     true ]
    ["YOUR_PERPLEXITY_API_KEY"    "PERPLEXITY_API_KEY" true ]
]

# Base names of live configs (relative to $HOME).
const LIVE_CONFIG_RELS = [
    [label                          rel];
    ["Antigravity (main)"           ".gemini/config/mcp_config.json"                ]
    ["Antigravity (alt)"            ".gemini/antigravity/mcp_config.json"           ]
    ["GitHub Copilot CLI"           ".copilot/mcp-config.json"                      ]
    ["Claude Code"                  ".claude.json"                                  ]
    ["OpenClaude"                   ".openclaude.json"                               ]
    ["Codex CLI"                    ".codex/config.toml"                            ]
    ["Grok"                         ".grok/config.toml"                             ]
    ["Kimi Code"                    ".kimi-code/config.toml"                        ]
    ["Gemini (settings)"            ".gemini/settings.json"                         ]
    ["Gemini (enablement)"          ".gemini/mcp-server-enablement.json"            ]
    ["Qwen Code"                    ".qwen/settings.json"                           ]
    ["OpenCode"                     ".config/opencode/opencode.jsonc"               ]
    ["Mimo Code"                    ".config/mimocode/mimocode.jsonc"               ]
    ["Goose"                        ".config/goose/config.yaml"                     ]
]

# Read an env var, returning null when unset or empty.
def env-value [name: string]: nothing -> any {
    let v = (try { $env | get $name } catch { null })
    if ($v | is-empty) { null } else { $v }
}

# Resolve every token to a value: env var first, then an interactive prompt, else null.
def resolve-tokens [interactive: bool]: nothing -> table {
    $TOKENS | each {|t|
        mut value = (env-value $t.env_var)
        if ($value == null) and $interactive {
            let entered = (
                if $t.secret {
                    input --suppress-output $"($t.env_var) [hidden, blank to skip]: "
                } else {
                    input $"($t.env_var) [blank to skip]: "
                }
            )
            if $t.secret { print "" }
            $value = (if ($entered | is-empty) { null } else { $entered })
        }
        {placeholder: $t.placeholder, value: $value, filled: ($value != null)}
    }
}

# Prompt for Y/N on the terminal; returns true for Yes (default).
def prompt-yn [label: string]: nothing -> bool {
    let answer = (input $"($label) [Y/n]: ")
    ($answer | str trim | is-empty) or ($answer | str downcase | str starts-with "y")
}

def main [
    --yes (-y)   # skip all Y/N prompts (auto-yes) for unattended/CI use
] {
    let home = ($env.HOME)
    let live_configs = ($LIVE_CONFIG_RELS | each {|c| {label: $c.label, path: ($home | path join $c.rel)} })

    let interactive = (
        not $yes
        and (is-terminal --stdin)
        and ((env-value "CI") == null)
        and ((env-value "CLAUDECODE") == null)
    )

    let tokens = (resolve-tokens $interactive)
    let active = ($tokens | where filled)

    # If no tokens were filled at all, nothing to do.
    if ($active | length) == 0 {
        print "No keys to fill — all env vars are unset."
        return
    }

    mut processed = 0
    mut skipped = 0

    for c in $live_configs {
        if not ($c.path | path exists) {
            print --stderr $"warning: ($c.label) not found at ($c.path) — skipping"
            continue
        }
        if $interactive and not (prompt-yn $"Fill keys in ($c.label) ($c.path)?") {
            $skipped += 1
            continue
        }
        mut content = (open --raw $c.path)
        for t in $active {
            $content = ($content | str replace --all $t.placeholder $t.value)
        }
        $content | save --force $c.path
        $processed += 1
    }

    print $"Filled ($active | length) of ($tokens | length) placeholders in ($processed) files"
    if $skipped > 0 { print $"  ($skipped) files declined" }
    print ($tokens | select placeholder filled)
}
