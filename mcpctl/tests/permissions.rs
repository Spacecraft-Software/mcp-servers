// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Permission tests for the two functions that put credentials on disk.
//!
//! Live configs hold real API keys, so every copy this tool makes of one holds them
//! too. The defaults do not: `create_dir_all` leaves 0755 and `fs::copy` preserves the
//! source's 0644, which is how three world-readable copies of a live Context7 key came
//! to sit in `~/.mcp-backup/` before anyone looked.

#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use mcpctl::deploy;

/// Permission bits of a path, masked to the usual nine.
fn mode(path: &Path) -> u32 {
    fs::metadata(path)
        .unwrap_or_else(|error| panic!("cannot stat `{}`: {error}", path.display()))
        .permissions()
        .mode()
        & 0o777
}

/// A scratch directory unique to one test, removed when the test ends.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("mcpctl-permissions-{name}"));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("scratch directory is creatable");
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_backup_is_readable_only_by_its_owner() {
    let scratch = Scratch::new("backup");
    let live = scratch.0.join("config.json");
    fs::write(&live, "{\"key\": \"ctx7sk-not-a-real-key\"}").expect("live config is writable");
    fs::set_permissions(&live, fs::Permissions::from_mode(0o644)).expect("mode is settable");

    let root = scratch.0.join(".mcp-backup").join("20260810T000000Z");
    deploy::backup(&live, &root).expect("backup succeeds");

    let copy = root.join(
        live.to_string_lossy()
            .trim_start_matches('/')
            .replace('/', "_"),
    );
    assert!(copy.exists(), "the backup copy was not created");

    assert_eq!(mode(&copy), 0o600, "the backup copy is not owner-only");
    assert_eq!(
        mode(&root),
        0o700,
        "the timestamp directory is not owner-only"
    );
    assert_eq!(
        mode(root.parent().expect("the vault is the copy's grandparent")),
        0o700,
        "the ~/.mcp-backup vault itself is not owner-only"
    );
}

#[test]
fn an_atomic_write_does_not_widen_the_file_it_replaces() {
    let scratch = Scratch::new("preserve");
    let live = scratch.0.join("config.json");
    fs::write(&live, "old").expect("live config is writable");
    // A user who has locked their config down must not have it reopened by a deploy.
    fs::set_permissions(&live, fs::Permissions::from_mode(0o600)).expect("mode is settable");

    deploy::write_atomically(&live, "new").expect("write succeeds");

    assert_eq!(fs::read_to_string(&live).expect("readable"), "new");
    assert_eq!(
        mode(&live),
        0o600,
        "the deploy widened a locked-down config"
    );
}

#[test]
fn an_atomic_write_leaves_no_temporary_behind() {
    let scratch = Scratch::new("temporary");
    let live = scratch.0.join("config.json");
    fs::write(&live, "old").expect("live config is writable");

    deploy::write_atomically(&live, "new").expect("write succeeds");

    let temporary = live.with_extension("mcpctl-tmp");
    assert!(
        !temporary.exists(),
        "`{}` survived the write, holding a copy of the credentials",
        temporary.display()
    );
}
