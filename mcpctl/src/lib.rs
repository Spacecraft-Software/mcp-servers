// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Manifest-driven generation and deployment of MCP server configs.
//!
//! This repository declares one logical set of MCP servers that must appear in every
//! host application's config, each in that host's own schema dialect. Historically both
//! the tracked templates and the live configs under `$HOME` were maintained by hand,
//! and the live half was routinely missed — a change could be correct in every template
//! while every host still ran the old invocation, with a clean `git status`.
//!
//! The pieces:
//!
//! - [`manifest`] — `mcp.toml`, the source of truth for *content*
//! - [`dialect`] — per-host schema *mechanics*, and normalization for comparison
//! - [`emit`] — JSON, TOML, and YAML serializers tuned to look hand-written
//! - [`render`] — manifest into host templates
//! - [`check`] — parity across the generated templates
//! - [`runtime`] — execution profile, output envelope, structured errors
//! - [`schema`] — the command surface as JSON Schema, for LLM function calling
//!
//! # Concurrency (Standard §3.2)
//!
//! Deliberately serial, and it should stay that way until a measurement says otherwise.
//! The whole workload is ~17 small files: fifteen are a few kilobytes, the largest is
//! `~/.claude.json` at 238 KB, and the work per file is a parse and a string splice.
//! Wall-clock is dominated by process start and by reading those files, not by compute,
//! so a Rayon `par_iter` over hosts would add a thread pool and a scheduling barrier to
//! save single-digit milliseconds.
//!
//! Concurrency would also make the risky part worse rather than better. `deploy` prompts
//! per file, prunes per stray, and writes with a backup and a re-parse; running that
//! concurrently would interleave prompts and make the order of writes — and therefore
//! the state after an interruption — nondeterministic. Priority 1 is Stability, and a
//! predictable sequence of file replacements is worth more here than a few milliseconds.
//!
//! Revisit if the host count grows by an order of magnitude, or if a future command has
//! to hash or diff large trees. The unit of parallelism would be the host, since hosts
//! share nothing.

pub mod check;
pub mod deploy;
pub mod dialect;
pub mod emit;
pub mod fill_keys;
pub mod manifest;
pub mod render;
pub mod runtime;
pub mod schema;
pub mod splice;
