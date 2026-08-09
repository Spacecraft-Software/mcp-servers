// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Parity checking across host templates.
//!
//! Three failure classes are reported:
//!
//! 1. a template that does not parse (this subsumes the former
//!    `.github/validate-configs.py`),
//! 2. a server declared by some hosts but not others, excluding the ones the manifest
//!    declares `omit = true` for that host,
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
use crate::manifest::Manifest;

/// A deliberate difference between one host and the rest.
///
/// Kept in code because it describes why a host *cannot* match — VS Code has no way to
/// hold a literal secret — rather than something the user chose. What the user chooses
/// belongs in the manifest, which is where class-2 omissions are read from.
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
    Variation {
        host: "VSCode",
        server: "github",
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

/// Servers a host does not declare, keyed by host.
type Absences = Vec<(String, Vec<String>)>;

/// Splits each host's undeclared servers into accidental and declared-absent.
///
/// Returns `(missing, omitted)`. Only `missing` fails the check; `omitted` is reported
/// so a deliberate gap is visible rather than merely silent.
fn classify_absences(
    parsed: &BTreeMap<&str, dialect::Servers>,
    union: &BTreeSet<String>,
    manifest: &Manifest,
) -> (Absences, Absences) {
    let mut missing: Absences = Vec::new();
    let mut omitted: Absences = Vec::new();

    for (host, servers) in parsed {
        let declared: BTreeSet<&str> = servers.names().collect();
        let excused = manifest.omitted_for(host);
        let (declared_absent, absent): (Vec<String>, Vec<String>) = union
            .iter()
            .filter(|name| !declared.contains(name.as_str()))
            .cloned()
            .partition(|name| excused.contains(name.as_str()));

        if !absent.is_empty() {
            missing.push(((*host).to_owned(), absent));
        }
        if !declared_absent.is_empty() {
            omitted.push(((*host).to_owned(), declared_absent));
        }
    }

    (missing, omitted)
}

/// Runs the parity check over every tracked template.
pub fn run(repo: &Path, profile: &Profile) -> Result<ExitCode, Failure> {
    // Class 2 needs the manifest: a server a host deliberately does not carry is not
    // missing. Unlike a template, an unreadable manifest is fatal rather than a finding
    // — nothing can be told apart from drift without it, so there is no honest report
    // to produce.
    let manifest = Manifest::load(repo).map_err(|error| {
        Failure::new(
            "MANIFEST_UNREADABLE",
            ExitCode::Failed,
            format!("{error:#}"),
            "mcpctl render --check --json",
        )
    })?;
    // A malformed or unreadable template is a finding, not a crash: `inspect` collects
    // them into the report so one bad file does not hide the state of the other twelve.
    Ok(inspect(repo, &manifest, profile))
}

/// Parses every template and reports how they disagree.
fn inspect(repo: &Path, manifest: &Manifest, profile: &Profile) -> ExitCode {
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

    let (missing, omitted) = classify_absences(&parsed, &union, manifest);

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
            "omitted": omitted
                .iter()
                .map(|(host, servers)| json!({
                    "host": host,
                    "servers": servers,
                    "reason": "declared `omit = true` in mcp.toml",
                }))
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
        print_report(&parsed, &union, &parse_failures, &missing, &omitted, &drift);
    }

    if ok { ExitCode::Ok } else { ExitCode::Failed }
}

/// Renders the human-readable report.
fn print_report(
    parsed: &BTreeMap<&str, dialect::Servers>,
    union: &BTreeSet<String>,
    parse_failures: &[(String, String)],
    missing: &[(String, Vec<String>)],
    omitted: &[(String, Vec<String>)],
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

    if !omitted.is_empty() {
        println!("\ndeclared omissions (mcp.toml):");
        for (host, servers) in omitted {
            println!("  {host} does not carry: {}", servers.join(", "));
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
        let omitted_count: usize = omitted.iter().map(|(_, servers)| servers.len()).sum();
        println!(
            "all hosts agree ({accepted_count} documented variations accepted, \
             {omitted_count} declared omissions)"
        );
    }
}
