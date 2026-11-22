//! Local-only, read-only metadata HUD for a modified Agent HUD build.
//!
//! Upstream is Agent HUD 0.6.11 (`jazzenchen/agent-hud-open`), Apache-2.0.
//! This package does not depend on the upstream host, platform, agent,
//! messaging, LAN, or share crates. It reads only paths the caller opts into,
//! and it keeps allowlisted counters and ids.

#![forbid(unsafe_code)]

mod claude_desktop;
mod codex;
mod cursor;
mod model;
mod policy;
mod render;
mod text;
mod time;

pub use model::{
    Group, OptIn, Quota, Report, RootKind, Row, Session, Snapshot, Tokens, Window, BUILD_LICENSE,
    BUILD_NAME, BUILD_UPSTREAM, BUILD_UPSTREAM_VERSION, BUILD_VERSION,
};
pub use render::{menu_bar, simple_hud};

use policy::Access;
use std::path::Path;

/// Read the opted-in roots and build the simple-HUD snapshot.
///
/// The process does not install an OS sandbox. `sandbox_applied` stays false.
/// A Mac wrapper can confine the process; this function does not claim that
/// happened.
pub fn collect(roots: &[OptIn]) -> Report {
    let mut access = Access::default();
    let mut rejected = Vec::new();
    let mut codex_scans = Vec::new();
    let mut claude_scans = Vec::new();
    let mut cursor_scans = Vec::new();
    let mut saw_codex = false;
    let mut saw_claude = false;
    let mut saw_cursor = false;

    for root in roots {
        if let Err(reason) = policy::check_root(&root.path) {
            rejected.push((root.path.clone(), reason));
            continue;
        }
        match root.kind {
            RootKind::CodexHome => {
                saw_codex = true;
                codex_scans.push(codex::scan(&mut access, &root.path));
            }
            RootKind::ClaudeDesktop => {
                saw_claude = true;
                claude_scans.push(claude_desktop::scan(&mut access, &root.path));
            }
            RootKind::CursorTranscripts => {
                saw_cursor = true;
                cursor_scans.push(cursor::scan(&mut access, &root.path));
            }
        }
    }

    let (codex_cli, chatgpt, unattributed) = codex::combine(&codex_scans, saw_codex);
    let claude = claude_desktop::combine(&claude_scans, saw_claude);
    let cursor_row = cursor::combine(&cursor_scans, saw_cursor);
    let mut snapshot = Snapshot {
        name: BUILD_NAME,
        version: BUILD_VERSION,
        upstream: BUILD_UPSTREAM,
        upstream_version: BUILD_UPSTREAM_VERSION,
        license: BUILD_LICENSE,
        modified: true,
        mode: "simple",
        notch: false,
        sandbox_applied: false,
        menu_bar: String::new(),
        rows: vec![codex_cli, chatgpt, claude, cursor_row],
        unattributed_codex: unattributed,
    };
    snapshot.menu_bar = menu_bar(&snapshot);
    Report {
        snapshot,
        opened: access.opened,
        skipped: access.skipped,
        rejected_roots: rejected,
    }
}

/// A root the caller named that this build refused to walk.
pub fn root_is_rejected(path: &Path, reason: &str) -> bool {
    policy::check_root(path).is_err_and(|found| found == reason)
}
