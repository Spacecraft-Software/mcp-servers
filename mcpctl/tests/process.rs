// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Stopping a process for `deploy --kill-running`, exercised on a throwaway child.
//!
//! The real target is a `claude` process, which no test may touch: matching by name
//! would reach the developer's own sessions. Each test therefore spawns its own `sleep`
//! and signals it by PID.

#![cfg(target_os = "linux")]

use std::process::{Child, Command};

use mcpctl::process::{Running, terminate};

/// A `sleep` child long enough to outlive any test.
fn sleeper() -> Child {
    Command::new("sleep")
        .arg("300")
        .spawn()
        .expect("`sleep` is available to spawn")
}

/// The child as `find` would report it.
fn running(child: &Child, ancestor: bool) -> Running {
    Running {
        pid: child.id(),
        name: "sleep".to_owned(),
        command: "sleep 300".to_owned(),
        ancestor,
    }
}

#[test]
fn a_running_process_is_stopped() {
    let mut child = sleeper();
    let survivors = terminate(&[running(&child, false)]);
    assert!(survivors.is_empty(), "still running: {survivors:?}");

    // SIGTERM is what ended it: `sleep` exits on it at once, so SIGKILL never ran.
    let status = child.wait().expect("the child can be reaped");
    assert!(!status.success());
}

#[test]
fn a_process_marked_as_an_ancestor_is_left_alone() {
    let mut child = sleeper();
    let process = running(&child, true);
    assert_eq!(terminate(std::slice::from_ref(&process)), vec![process.pid]);
    assert!(child.try_wait().expect("the child can be polled").is_none());

    child.kill().expect("the test cleans up its own child");
    let _ = child.wait();
}

#[test]
fn a_pid_whose_name_no_longer_matches_is_not_signalled() {
    let mut child = sleeper();
    let mut process = running(&child, false);
    // The PID now "belongs to" a differently named process, as after PID reuse.
    process.name = "claude".to_owned();
    assert!(
        terminate(&[process]).is_empty(),
        "nothing matching survives"
    );
    assert!(
        child.try_wait().expect("the child can be polled").is_none(),
        "a process with another name must not be signalled"
    );

    child.kill().expect("the test cleans up its own child");
    let _ = child.wait();
}
