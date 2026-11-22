//! Snapshot types. Strings here are allowlisted ids, models, or fixed reasons.
//! Session prompts, tool output, and source text have no field to land in.

use serde::Serialize;
use std::path::PathBuf;

pub const BUILD_NAME: &str = "agent-hud-local";
pub const BUILD_VERSION: &str = "0.6.11-local.1";
pub const BUILD_UPSTREAM: &str = "https://github.com/jazzenchen/agent-hud-open";
pub const BUILD_UPSTREAM_VERSION: &str = "0.6.11";
pub const BUILD_LICENSE: &str = "Apache-2.0";

pub mod reasons {
    pub const NO_PATH: &str = "no path opted in";
    pub const CODEX_QUOTA: &str = "no client-recorded rate_limits in the opted-in logs";
    pub const CODEX_TOKENS: &str = "no client-recorded token counters in the opted-in logs";
    pub const CLAUDE_QUOTA: &str = "Claude Desktop plan quota is served by claude.ai and needs the app sign-in; this build does not read cookies or use the network";
    pub const CLAUDE_TOKENS: &str =
        "the Claude Desktop session index has no allowlisted token counters";
    pub const CLAUDE_SHAPE: &str = "opted-in path has no Claude Desktop session index";
    pub const CURSOR_QUOTA: &str = "Cursor plan quota needs the account database and a network call; this build does not open that database or use the network";
    pub const CURSOR_TOKENS: &str = "Cursor generation tokens are not stored as separate metadata; this build does not install hooks or open transcript text";
    pub const CURSOR_SHAPE: &str = "opt-in must be a directory named agent-transcripts; this build does not walk a Cursor application-support folder";
    pub const SYMLINK_ROOT: &str = "opted-in path is a symlink and was not followed";
    pub const NOT_A_DIRECTORY: &str = "opted-in path is not a directory";
    pub const MISSING: &str = "opted-in path is missing";
    pub const DENIED_ROOT: &str = "opted-in path uses a denied name";
    pub const FILESYSTEM_ROOT: &str = "opted-in path is a filesystem root";
    pub const UNREADABLE: &str = "opted-in path is unreadable";
    pub const NONE: &str = "none in the opted-in path";
    pub const LISTED: &str = "ids and counters only";
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RootKind {
    /// A Codex home: `sessions/` and `archived_sessions/` rollouts.
    /// ChatGPT desktop writes the same rollout shape; each file is classified
    /// from `session_meta` and never from the directory name alone.
    CodexHome,
    ClaudeDesktop,
    CursorTranscripts,
}

#[derive(Clone, Debug)]
pub struct OptIn {
    pub kind: RootKind,
    pub path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skipped {
    pub path: PathBuf,
    pub reason: &'static str,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Report {
    pub snapshot: Snapshot,
    #[serde(skip)]
    pub opened: Vec<PathBuf>,
    #[serde(skip)]
    pub skipped: Vec<Skipped>,
    #[serde(skip)]
    pub rejected_roots: Vec<(PathBuf, &'static str)>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Snapshot {
    pub name: &'static str,
    pub version: &'static str,
    pub upstream: &'static str,
    pub upstream_version: &'static str,
    pub license: &'static str,
    pub modified: bool,
    pub mode: &'static str,
    pub notch: bool,
    /// This process never calls an OS sandbox API. A wrapper may confine it.
    pub sandbox_applied: bool,
    pub menu_bar: String,
    pub rows: Vec<Row>,
    pub unattributed_codex: Group,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Row {
    pub product: &'static str,
    pub quota: Quota,
    pub tokens: Tokens,
    pub sessions: Sessions,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Group {
    pub count: usize,
    pub truncated: bool,
    pub quota: Quota,
    pub tokens: Tokens,
    pub items: Vec<Session>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Quota {
    Unavailable { reason: &'static str },
    ClientRecorded { windows: Vec<Window> },
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Window {
    pub slot: &'static str,
    pub used_percent: f64,
    pub window_minutes: u64,
    pub resets_at_unix: i64,
    pub plan_type: Option<String>,
    pub observed_unix: Option<i64>,
    pub source: &'static str,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Tokens {
    Unavailable {
        reason: &'static str,
    },
    Recorded {
        input_tokens: u64,
        output_tokens: u64,
        cached_input_tokens: u64,
        cache_write_input_tokens: u64,
        reasoning_output_tokens: u64,
        sessions_with_counters: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Sessions {
    pub available: bool,
    pub reason: &'static str,
    pub count: usize,
    pub truncated: bool,
    pub items: Vec<Session>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Session {
    pub id: String,
    pub model: Option<String>,
    pub archived: bool,
    pub subagent: bool,
    pub place: Option<&'static str>,
    pub observed_unix: Option<i64>,
    pub activity_unix: Option<i64>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cached_input_tokens: Option<u64>,
    pub cache_write_input_tokens: Option<u64>,
    pub reasoning_output_tokens: Option<u64>,
}

pub fn empty_row(product: &'static str, reason: &'static str) -> Row {
    Row {
        product,
        quota: Quota::Unavailable { reason },
        tokens: Tokens::Unavailable { reason },
        sessions: Sessions {
            available: false,
            reason,
            count: 0,
            truncated: false,
            items: Vec::new(),
        },
    }
}

pub fn listed_sessions(items: Vec<Session>, truncated: bool) -> Sessions {
    let count = items.len();
    Sessions {
        available: true,
        reason: if count == 0 {
            reasons::NONE
        } else {
            reasons::LISTED
        },
        count,
        truncated,
        items,
    }
}
