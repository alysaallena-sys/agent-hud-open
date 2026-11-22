//! Cursor session ids from an opted-in `agent-transcripts` directory.
//! Transcript bytes and `state.vscdb` are not opened. Quota and token totals
//! stay unavailable: both need credentials, hooks, or the network.

use crate::model::{empty_row, listed_sessions, reasons, Quota, Row, Session, Tokens};
use crate::policy::{Access, DISPLAY_CAP};
use crate::text::uuid;
use std::path::Path;

pub struct Scan {
    sessions: Vec<Session>,
    shaped: bool,
    truncated: bool,
}

pub fn scan(access: &mut Access, root: &Path) -> Scan {
    if root.file_name().and_then(|name| name.to_str()) != Some("agent-transcripts") {
        return Scan {
            sessions: Vec::new(),
            shaped: false,
            truncated: false,
        };
    }
    let mut sessions = Vec::new();
    for child in access.list(root, Path::new("")) {
        let Some(name) = child.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !uuid(name) {
            continue;
        }
        if !access.stat(root, &child).is_some_and(|stat| stat.is_dir) {
            continue;
        }
        let transcript = child.join(format!("{name}.jsonl"));
        let stat = access
            .stat(root, &transcript)
            .filter(|stat| stat.is_file)
            .or_else(|| access.stat(root, &child));
        sessions.push(Session {
            id: name.to_owned(),
            model: None,
            archived: false,
            subagent: false,
            place: None,
            observed_unix: None,
            activity_unix: stat.and_then(|stat| stat.modified_unix),
            input_tokens: None,
            output_tokens: None,
            cached_input_tokens: None,
            cache_write_input_tokens: None,
            reasoning_output_tokens: None,
        });
    }
    sessions.sort_by(|left, right| {
        right
            .activity_unix
            .cmp(&left.activity_unix)
            .then_with(|| left.id.cmp(&right.id))
    });
    let truncated = sessions.len() > DISPLAY_CAP;
    sessions.truncate(DISPLAY_CAP);
    Scan {
        sessions,
        shaped: true,
        truncated,
    }
}

pub fn combine(scans: &[Scan], saw_root: bool) -> Row {
    if !saw_root {
        return empty_row("Cursor", reasons::NO_PATH);
    }
    if !scans.iter().any(|scan| scan.shaped) {
        return Row {
            product: "Cursor",
            quota: Quota::Unavailable {
                reason: reasons::CURSOR_QUOTA,
            },
            tokens: Tokens::Unavailable {
                reason: reasons::CURSOR_TOKENS,
            },
            sessions: crate::claude_desktop::unavailable_sessions(reasons::CURSOR_SHAPE),
        };
    }
    let mut sessions = Vec::new();
    let mut truncated = false;
    for scan in scans {
        truncated |= scan.truncated;
        for session in &scan.sessions {
            if sessions.iter().any(|kept: &Session| kept.id == session.id) {
                continue;
            }
            sessions.push(session.clone());
        }
    }
    Row {
        product: "Cursor",
        quota: Quota::Unavailable {
            reason: reasons::CURSOR_QUOTA,
        },
        tokens: Tokens::Unavailable {
            reason: reasons::CURSOR_TOKENS,
        },
        sessions: listed_sessions(sessions, truncated),
    }
}
