// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Parity checking across host templates.
//!
//! Three failure classes are reported:
//!
//! 1. a template that does not parse (this subsumes the former
//!    `.github/validate-configs.py`),
//! 2. a server declared by some hosts but not others,
//! 3. a server whose normalized invocation differs between hosts.
//!
//! Class 3 is the one that matters and the one a naive checker gets wrong. Comparing
//! raw keys reports opencode's `environment` as "missing `env`" — a false positive that
//! an early throwaway script produced. Comparison therefore runs on
//! [`Transport`](crate::dialect::Transport) values, after dialect normalization.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::json;

use crate::runtime::{ExitCode, Failure, Profile};

use crate::dialect::{self, HOSTS, Strictness, Transport};

/// A deliberate difference between one host and the rest.
///
/// Kept in code for now because it describes why a host *cannot* match, not what the
/// user wants declared; `mcpctl render` will additionally enforce these from the
/// manifest's per-host overrides.
#[derive(Debug, Clone, Copy)]
struct Variation {
    /// Host that differs.
    host: &'static str,
    /// Server whose invocation differs.
    server: &'static str,
    /// Why the difference is correct.
    reason: &'static str,
}

/// Differences that are correct and must not fail the check.
const ACCEPTED: &[Variation] = &[
    Variation {
        host: "Antigravity",
        server: "nixos",
        reason: "runs the mcp-nixos binary directly rather than through `nix run`",
    },
    Variation {
        host: "VSCode",
        server: "context7",
        reason: "resolves the key through a prompted `input`, not a literal placeholder",
    },
    Variation {
        host: "VSCode",
        server: "brave-search",
        reason: "resolves the key through a prompted `input`, not a literal placeholder",
    },
    Variation {
        host: "VSCode",
        server: "perplexity",
        reason: "resolves the key through a prompted `input`, not a literal placeholder",
    },
];

/// Whether a host/server pair is a documented, accepted difference.
fn accepted(host: &str, server: &str) -> Option<&'static str> {
    ACCEPTED
        .iter()
        .find(|variation| variation.host == host && variation.server == server)
        .map(|variation| variation.reason)
}

/// One server's invocation as each host spells it.
type ByHost = BTreeMap<String, Transport>;

/// Competing invocations for one server: each distinct invocation and the hosts using it.
type Variants = Vec<(Transport, Vec<String>)>;

/// Servers whose hosts disagree, paired with the competing invocations.
type Drift = Vec<(String, Variants)>;

/// Runs the parity check over every tracked template.
pub fn run(repo: &Path, profile: &Profile) -> Result<ExitCode, Failure> {
    // A malformed or unreadable template is a finding, not a crash: `inspect` collects
    // them into the report so one bad file does not hide the state of the other twelve.
    Ok(inspect(repo, profile))
}

/// Parses every template and reports how they disagree.
fn inspect(repo: &Path, profile: &Profile) -> ExitCode {
    let mut parsed: BTreeMap<&str, dialect::Servers> = BTreeMap::new();
    let mut parse_failures: Vec<(String, String)> = Vec::new();

    for host in HOSTS {
        let path = repo.join(host.template);
        match dialect::load(&path, host, Strictness::Strict) {
            Ok(servers) => {
                parsed.insert(host.name, servers);
            }
            // A malformed template is reported rather than aborting, so one bad file
            // does not hide the state of the other twelve.
            Err(error) => parse_failures.push((host.name.to_owned(), format!("{error:#}"))),
        }
    }

    let union: BTreeSet<String> = parsed
        .values()
        .flat_map(|servers| servers.names().map(str::to_owned))
        .collect();

    let mut missing: Vec<(String, Vec<String>)> = Vec::new();
    for (host, servers) in &parsed {
        let declared: BTreeSet<&str> = servers.names().collect();
        let absent: Vec<String> = union
            .iter()
            .filter(|name| !declared.contains(name.as_str()))
            .cloned()
            .collect();
        if !absent.is_empty() {
            missing.push(((*host).to_owned(), absent));
        }
    }

    let mut drift: Drift = Vec::new();
    for server in &union {
        let by_host: ByHost = parsed
            .iter()
            .filter_map(|(host, servers)| {
                servers
                    .get(server)
                    .map(|transport| ((*host).to_owned(), transport.clone()))
            })
            .collect();

        // Group hosts by identical invocation; one group means full agreement.
        let mut groups: Variants = Vec::new();
        for (host, transport) in by_host {
            match groups.iter_mut().find(|(known, _)| *known == transport) {
                Some((_, hosts)) => hosts.push(host),
                None => groups.push((transport, vec![host])),
            }
        }

        if groups.len() > 1 {
            // A minority group is drift unless every host in it is an accepted variation.
            groups.sort_by_key(|(_, hosts)| std::cmp::Reverse(hosts.len()));
            let unexplained = groups
                .iter()
                .skip(1)
                .any(|(_, hosts)| hosts.iter().any(|host| accepted(host, server).is_none()));
            if unexplained {
                drift.push((server.clone(), groups));
            }
        }
    }

    let ok = parse_failures.is_empty() && missing.is_empty() && drift.is_empty();

    if profile.json {
        let report = json!({
            "ok": ok,
            "templates_checked": HOSTS.len(),
            "templates_parsed": parsed.len(),
            "servers": union.iter().collect::<Vec<_>>(),
            "parse_failures": parse_failures
                .iter()
                .map(|(host, error)| json!({ "host": host, "error": error }))
                .collect::<Vec<_>>(),
            "missing": missing
                .iter()
                .map(|(host, servers)| json!({ "host": host, "servers": servers }))
                .collect::<Vec<_>>(),
            "drift": drift
                .iter()
                .map(|(server, groups)| json!({
                    "server": server,
                    "variants": groups
                        .iter()
                        .map(|(transport, hosts)| json!({
                            "invocation": transport.summary(),
                            "hosts": hosts,
                        }))
                        .collect::<Vec<_>>(),
                }))
                .collect::<Vec<_>>(),
        });
        profile.emit("mcpctl check", &report);
    } else {
        print_report(&parsed, &union, &parse_failures, &missing, &drift);
    }

    if ok { ExitCode::Ok } else { ExitCode::Failed }
}

/// Renders the human-readable report.
fn print_report(
    parsed: &BTreeMap<&str, dialect::Servers>,
    union: &BTreeSet<String>,
    parse_failures: &[(String, String)],
    missing: &[(String, Vec<String>)],
    drift: &[(String, Variants)],
) {
    println!(
        "{}/{} templates parsed, {} servers declared",
        parsed.len(),
        HOSTS.len(),
        union.len()
    );

    if !parse_failures.is_empty() {
        println!("\nparse failures:");
        for (host, error) in parse_failures {
            println!("  {host}: {error}");
        }
    }

    if !missing.is_empty() {
        println!("\nincomplete hosts:");
        for (host, servers) in missing {
            println!("  {host} is missing: {}", servers.join(", "));
        }
    }

    if !drift.is_empty() {
        println!("\ninvocation drift:");
        for (server, groups) in drift {
            println!("  {server}:");
            for (transport, hosts) in groups {
                println!("    [{:>2}] {}", hosts.len(), hosts.join(", "));
                println!("         {}", transport.summary());
            }
        }
    }

    if parse_failures.is_empty() && missing.is_empty() && drift.is_empty() {
        let accepted_count = ACCEPTED.len();
        println!("all hosts agree ({accepted_count} documented variations accepted)");
    }
}
