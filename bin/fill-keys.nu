#!/usr/bin/env nu
# SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
# SPDX-License-Identifier: GPL-3.0-or-later
#
# fill-keys.nu — fill the MCP config templates' placeholders with real values and
# write ready-to-use copies into a gitignored output dir (default: dist/).
#
# Values are read from environment variables; any that are unset are prompted for
# (hidden input) when run interactively, or left as placeholders when not (CI/agent).
# The tracked templates are never modified — secrets only ever land in the output dir.
#
#   Env var          Fills placeholder           Server
#   CONTEXT7_API_KEY  YOUR_CONTEXT7_API_KEY       context7
#   BRAVE_API_KEY     YOUR_BRAVE_API_KEY          brave-search
#   GITHUB_PAT        YOUR_GITHUB_PAT             github
#   (filesystem path is now hardcoded to /spacecraft-software in the templates)

#
# Usage:
#   CONTEXT7_API_KEY=ctx7sk-... BRAVE_API_KEY=... nu bin/fill-keys.nu
#   nu bin/fill-keys.nu --out /tmp/mcp        # custom output dir
#   nu bin/fill-keys.nu --help

# Each placeholder, the env var that fills it, and whether it is a secret (hidden prompt).
const TOKENS = [
    [placeholder                  env_var            secret];
    ["YOUR_CONTEXT7_API_KEY"      "CONTEXT7_API_KEY"  true ]
    ["YOUR_BRAVE_API_KEY"         "BRAVE_API_KEY"     true ]
    ["YOUR_GITHUB_PAT"            "GITHUB_PAT"        true ]
]

# Host config templates and where each filled copy is meant to be installed.
# VS Code and the Gemini enablement file carry no placeholders — they are copied verbatim.
const HOSTS = [
    [src                                       live];
    ["Antigravity/mcp_config.json"             "(Antigravity — editor-managed)"        ]
    ["VSCode/mcp.json"                         "<workspace>/.vscode/mcp.json"          ]
    ["GitHubCopilotCLI/mcp-config.json"        "~/.copilot/mcp-config.json"            ]
    ["ClaudeCode/.mcp.json"                    "~/.claude.json (mcpServers)"           ]
    ["OpenClaude/.mcp.json"                    "~/.openclaude.json (mcpServers)"       ]
    ["Codex/config.toml"                       "~/.codex/config.toml"                  ]
    ["Grok/config.toml"                        "~/.grok/config.toml"                   ]
    ["Kimi/config.toml"                        "~/.kimi-code/config.toml"              ]
    ["Gemini/settings.json"                    "~/.gemini/settings.json"               ]
    ["Gemini/mcp-server-enablement.json"       "~/.gemini/mcp-server-enablement.json"  ]
    ["Qwen/settings.json"                       "~/.qwen/settings.json"                 ]
    ["OpenCode/opencode.jsonc"                 "~/.config/opencode/opencode.jsonc"     ]
    ["Mimo/mimocode.jsonc"                     "~/.config/mimocode/mimocode.jsonc"     ]
    ["Goose/config.yaml"                       "~/.config/goose/config.yaml"           ]
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
            # `input --suppress-output` leaves the cursor on the prompt line.
            if $t.secret { print "" }
            $value = (if ($entered | is-empty) { null } else { $entered })
        }
        {placeholder: $t.placeholder, value: $value, filled: ($value != null)}
    }
}

# Fill placeholders in the templates and write copies under `out`.
def main [
    --out: string = "dist"   # output directory (gitignored), relative to the repo root
] {
    let repo = ($env.FILE_PWD | path dirname)
    let interactive = (
        (is-terminal --stdin)
        and ((env-value "CI") == null)
        and ((env-value "CLAUDECODE") == null)
    )

    let tokens = (resolve-tokens $interactive)
    let active = ($tokens | where filled)

    let out_root = if ($out | path type) == "dir" or ($out | str starts-with "/") {
        $out
    } else {
        $repo | path join $out
    }

    for h in $HOSTS {
        let src = ($repo | path join $h.src)
        if not ($src | path exists) {
            print --stderr $"error: missing template ($h.src)"
            exit 1
        }
        let dst = ($out_root | path join $h.src)
        mkdir ($dst | path dirname)
        mut content = (open --raw $src)
        for t in $active {
            $content = ($content | str replace --all $t.placeholder $t.value)
        }
        $content | save --force $dst
    }

    # Summary to stdout.
    print $"Filled ($active | length) of ($tokens | length) placeholders → ($out_root)/"
    print ($tokens | select placeholder filled)
    let skipped = ($tokens | where not filled | get placeholder)
    if ($skipped | is-not-empty) {
        print $"Left as placeholders — those servers stay inert: ($skipped | str join ', ')"
    }
    print ""
    print "Install destinations:"
    print ($HOSTS | select src live)
}
