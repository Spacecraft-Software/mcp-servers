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

pub mod check;
pub mod deploy;
pub mod dialect;
pub mod emit;
pub mod fill_keys;
pub mod manifest;
pub mod render;
pub mod splice;
