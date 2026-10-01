// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Finding, and on request stopping, the host processes that own their configs.
//!
//! Claude Code rewrites `~/.claude.json` on exit, so a deploy written underneath a
//! running instance is silently reverted. `deploy` therefore refuses a host whose process
//! is running, and `deploy --kill-running` offers to stop those processes first.
//!
//! "Running" is easy to get wrong in a way the user cannot see: a `claude --resume` left
//! in a detached multiplexer pane, or a session in another workspace, is still a
//! `claude` process. That is why every process this module finds is reported with its
//! PID and full command line before anything is asked.
//!
//! Everything here reads `/proc` directly rather than shelling out to `pgrep`, which is
//! not present everywhere. Where `/proc` does not exist, nothing is found, and the caller
//! falls back to the confirmation prompt.

use std::collections::BTreeSet;
use std::path::Path;
use std::time::{Duration, Instant};

/// How long a process gets to exit after `SIGTERM` before it is sent `SIGKILL`.
///
/// Claude Code saves its session and rewrites `~/.claude.json` while it shuts down —
/// which is the write a deploy must land after. Five seconds covers that on this
/// machine with a wide margin; shortening it risks cutting the save off with `SIGKILL`.
const TERM_GRACE: Duration = Duration::from_secs(5);

/// How long to wait for `SIGKILL` to take effect before reporting a survivor.
///
/// The kernel acts on `SIGKILL` at once; this only covers a process stuck in
/// uninterruptible sleep (state `D`), which no signal can end.
const KILL_GRACE: Duration = Duration::from_secs(2);

/// How often to re-check whether the signalled processes are gone.
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// One process whose name matched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Running {
    /// Process ID.
    pub pid: u32,
    /// The process name the match was made on, as the kernel reports it in `comm`.
    pub name: String,
    /// The full command line, arguments separated by spaces.
    pub command: String,
    /// Whether this process is an ancestor of this one — the shell or session `mcpctl`
    /// was started from. An ancestor is never signalled.
    pub ancestor: bool,
}

/// Every running process with this name, in ascending PID order.
pub fn find(name: &str) -> Vec<Running> {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    let ancestors = ancestors();
    let mut found: Vec<Running> = entries
        .flatten()
        .filter_map(|entry| entry.file_name().to_str()?.parse::<u32>().ok())
        .filter(|&pid| alive(pid) && comm(pid).is_some_and(|comm| comm == name))
        .map(|pid| Running {
            pid,
            name: name.to_owned(),
            command: command_line(pid),
            ancestor: ancestors.contains(&pid),
        })
        .collect();
    found.sort_by_key(|process| process.pid);
    found
}

/// Stops each process with `SIGTERM`, then `SIGKILL` for any still alive after
/// [`TERM_GRACE`].
///
/// Returns the PIDs still running afterwards. Ancestors are skipped and reported as
/// survivors, as is any PID whose name no longer matches — the original process has
/// exited and the number now belongs to something else, which must not be signalled.
pub fn terminate(processes: &[Running]) -> Vec<u32> {
    let mut survivors: Vec<u32> = processes
        .iter()
        .filter(|process| process.ancestor)
        .map(|process| process.pid)
        .collect();
    let targets: Vec<&Running> = processes
        .iter()
        .filter(|process| !process.ancestor)
        .collect();

    for signal in [Signal::Terminate, Signal::Kill] {
        let pending: Vec<&Running> = targets
            .iter()
            .copied()
            .filter(|process| still(process))
            .collect();
        if pending.is_empty() {
            break;
        }
        for process in &pending {
            send(process.pid, signal);
        }
        let grace = match signal {
            Signal::Terminate => TERM_GRACE,
            Signal::Kill => KILL_GRACE,
        };
        wait_for_exit(&pending, grace);
    }

    survivors.extend(
        targets
            .iter()
            .filter(|process| still(process))
            .map(|process| process.pid),
    );
    survivors.sort_unstable();
    survivors
}

/// The PIDs of this process's ancestors, up to and including PID 1.
fn ancestors() -> BTreeSet<u32> {
    let mut found = BTreeSet::new();
    let mut pid = std::process::id();
    while let Some(parent) = parent(pid) {
        // PID 0 is the kernel's "no parent"; a repeat means /proc changed underneath
        // the walk, and stopping is the only safe answer to either.
        if parent == 0 || !found.insert(parent) {
            break;
        }
        pid = parent;
    }
    found
}

/// The fields of `/proc/<pid>/stat` after the command name.
///
/// The name sits in parentheses and may itself contain spaces and `)`, so the split is
/// made at the *last* `)`, not by counting whitespace from the start.
fn stat_fields(pid: u32) -> Option<Vec<String>> {
    let stat =
        std::fs::read_to_string(Path::new("/proc").join(pid.to_string()).join("stat")).ok()?;
    let (_, rest) = stat.rsplit_once(')')?;
    Some(rest.split_whitespace().map(str::to_owned).collect())
}

/// A process's parent PID.
fn parent(pid: u32) -> Option<u32> {
    stat_fields(pid)?.get(1)?.parse().ok()
}

/// Whether a process exists and has not yet exited.
///
/// A zombie (`Z`) or dead (`X`) process still has a `/proc` entry until its parent reaps
/// it, but it no longer holds any file open and will write nothing more, which is all a
/// deploy cares about.
fn alive(pid: u32) -> bool {
    stat_fields(pid)
        .and_then(|fields| fields.first().cloned())
        .is_some_and(|state| state != "Z" && state != "X")
}

/// A process's name as the kernel reports it, without the trailing newline.
fn comm(pid: u32) -> Option<String> {
    std::fs::read_to_string(Path::new("/proc").join(pid.to_string()).join("comm"))
        .ok()
        .map(|comm| comm.trim_end().to_owned())
}

/// A process's command line, arguments joined by spaces; empty for a kernel thread.
fn command_line(pid: u32) -> String {
    std::fs::read(Path::new("/proc").join(pid.to_string()).join("cmdline"))
        .map(|bytes| {
            bytes
                .split(|&byte| byte == 0)
                .filter(|argument| !argument.is_empty())
                .map(String::from_utf8_lossy)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default()
}

/// Whether this exact process is still running: alive, and still carrying its name.
fn still(process: &Running) -> bool {
    alive(process.pid) && comm(process.pid).is_some_and(|comm| comm == process.name)
}

/// Polls until every process has exited or the grace period runs out.
fn wait_for_exit(processes: &[&Running], grace: Duration) {
    let deadline = Instant::now() + grace;
    while processes.iter().any(|process| still(process)) && Instant::now() < deadline {
        std::thread::sleep(POLL_INTERVAL);
    }
}

/// The two signals this module sends.
#[derive(Debug, Clone, Copy)]
enum Signal {
    /// `SIGTERM`: ask the process to save and exit.
    Terminate,
    /// `SIGKILL`: end it now.
    Kill,
}

/// Sends a signal to one process. A failure is not reported here: the caller re-checks
/// whether the process is gone, which is the only outcome that matters.
#[cfg(unix)]
fn send(pid: u32, signal: Signal) {
    // A PID of 0 or below addresses a process *group* (or every process) rather than one
    // process. `find` never yields one, but this is the line that would do the damage,
    // so it refuses here rather than trusting the caller.
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return;
    };
    if pid <= 0 {
        return;
    }
    let signal = match signal {
        Signal::Terminate => libc::SIGTERM,
        Signal::Kill => libc::SIGKILL,
    };
    // SAFETY: kill(2) takes two plain integers and reads or writes no memory owned by
    // this program. `pid` is positive, so it names exactly one process, never a group.
    let _ = unsafe { libc::kill(pid, signal) };
}

/// Signals are a Unix concept; elsewhere nothing is found to signal in the first place.
#[cfg(not(unix))]
fn send(_pid: u32, _signal: Signal) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_process_is_alive_and_not_its_own_ancestor() {
        let pid = std::process::id();
        assert!(alive(pid));
        assert!(!ancestors().contains(&pid));
    }

    #[test]
    fn the_parent_is_an_ancestor() {
        let parent = parent(std::process::id()).expect("a running test has a parent");
        assert!(ancestors().contains(&parent));
    }

    #[test]
    fn ancestors_are_never_signalled() {
        let process = Running {
            pid: parent(std::process::id()).expect("a running test has a parent"),
            name: comm(std::process::id()).unwrap_or_default(),
            command: String::new(),
            ancestor: true,
        };
        assert_eq!(terminate(std::slice::from_ref(&process)), vec![process.pid]);
        assert!(alive(process.pid), "the parent must survive");
    }
}
