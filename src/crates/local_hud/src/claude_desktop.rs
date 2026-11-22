//! Claude Desktop session index only. Cookies, config.json, and Claude Code
//! transcripts are not opened. Plan quota stays unavailable.

use crate::json_walk::for_each_value;
use crate::model::{empty_row, listed_sessions, reasons, Quota, Row, Session, Sessions, Tokens};
use crate::policy::{Access, DISPLAY_CAP, MAX_METADATA};
use crate::text::{is_token, uuid};
use std::path::Path;

pub struct Scan {
    sessions: Vec<Session>,
    truncated: bool,
    shaped: bool,
}

pub fn scan(access: &mut Access, root: &Path) -> Scan {
    let mut sessions = Vec::new();
    let mut shaped = false;
    for (folder, place) in [
        ("claude-code-sessions", "code tab"),
        ("local-agent-mode-sessions", "cowork"),
    ] {
        if access
            .stat(root, Path::new(folder))
            .is_some_and(|stat| stat.is_dir)
        {
            shaped = true;
            sessions.extend(folder_sessions(access, root, folder, place));
        }
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
        truncated,
        shaped,
    }
}

pub fn combine(scans: &[Scan], saw_root: bool) -> Row {
    if !saw_root {
        return empty_row("Claude Desktop", reasons::NO_PATH);
    }
    if !scans.iter().any(|scan| scan.shaped) {
        return empty_row("Claude Desktop", reasons::CLAUDE_SHAPE);
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
    let tokens = sum_tokens(&sessions);
    Row {
        product: "Claude Desktop",
        quota: Quota::Unavailable {
            reason: reasons::CLAUDE_QUOTA,
        },
        tokens,
        sessions: listed_sessions(sessions, truncated),
    }
}

fn sum_tokens(sessions: &[Session]) -> Tokens {
    let mut input = 0_u64;
    let mut output = 0_u64;
    let mut cache_read = 0_u64;
    let mut cache_write = 0_u64;
    let mut reasoning = 0_u64;
    let mut sessions_with_counters = 0_u64;
    for session in sessions {
        let Some(value) = session.input_tokens else {
            continue;
        };
        sessions_with_counters += 1;
        input = input.saturating_add(value);
        output = output.saturating_add(session.output_tokens.unwrap_or(0));
        cache_read = cache_read.saturating_add(session.cached_input_tokens.unwrap_or(0));
        cache_write = cache_write.saturating_add(session.cache_write_input_tokens.unwrap_or(0));
        reasoning = reasoning.saturating_add(session.reasoning_output_tokens.unwrap_or(0));
    }
    if sessions_with_counters == 0 {
        return Tokens::Unavailable {
            reason: reasons::CLAUDE_TOKENS,
        };
    }
    Tokens::Recorded {
        input_tokens: input,
        output_tokens: output,
        cached_input_tokens: cache_read,
        cache_write_input_tokens: cache_write,
        reasoning_output_tokens: reasoning,
        sessions_with_counters,
    }
}

fn folder_sessions(
    access: &mut Access,
    root: &Path,
    folder: &str,
    place: &'static str,
) -> Vec<Session> {
    let mut found = Vec::new();
    for org in access.list(root, Path::new(folder)) {
        if !ident_component(org.file_name()) {
            continue;
        }
        for account in access.list(root, &org) {
            if !ident_component(account.file_name()) {
                continue;
            }
            for file in access.list(root, &account) {
                let Some(name) = file.file_name().and_then(|name| name.to_str()) else {
                    continue;
                };
                let Some(id) = local_id(name) else {
                    continue;
                };
                if let Some(session) = read_index(access, root, &file, &id, place) {
                    found.push(session);
                }
            }
        }
    }
    found
}

fn read_index(
    access: &mut Access,
    root: &Path,
    rel: &Path,
    id: &str,
    place: &'static str,
) -> Option<Session> {
    let stat = access.stat(root, rel)?;
    if !stat.is_file || stat.len > MAX_METADATA {
        return None;
    }
    let file = access.open(root, rel)?;
    let mut session_id = None;
    let mut archived = false;
    let mut tainted = false;
    let mut input = None;
    let mut output = None;
    let mut cache_read = None;
    let mut cache_write = None;
    let mut reasoning = None;
    for_each_value(file, |hits| {
        for hit in hits {
            match hit.path.as_str() {
                "sessionId" => session_id = hit.text.clone(),
                "isArchived" => archived = hit.flag == Some(true),
                "usage.input_tokens" => input = whole(hit.number),
                "usage.output_tokens" => output = whole(hit.number),
                "usage.cached_input_tokens" => cache_read = whole(hit.number),
                "usage.cache_write_input_tokens" => cache_write = whole(hit.number),
                "usage.reasoning_output_tokens" => reasoning = whole(hit.number),
                path if path.starts_with("usage.") => tainted = true,
                _ => {}
            }
        }
    });
    if session_id.as_deref() != Some(id) {
        return None;
    }
    let usage = (!tainted).then_some(()).and_then(|_| {
        Some([
            input?,
            output?,
            cache_read.unwrap_or(0),
            cache_write.unwrap_or(0),
            reasoning.unwrap_or(0),
        ])
    });
    Some(Session {
        id: id.to_owned(),
        model: None,
        archived,
        subagent: false,
        place: Some(place),
        observed_unix: None,
        activity_unix: stat.modified_unix,
        input_tokens: usage.map(|usage| usage[0]),
        output_tokens: usage.map(|usage| usage[1]),
        cached_input_tokens: usage.map(|usage| usage[2]),
        cache_write_input_tokens: usage.map(|usage| usage[3]),
        reasoning_output_tokens: usage.map(|usage| usage[4]),
    })
}

fn whole(number: Option<f64>) -> Option<u64> {
    let number = number?;
    (number >= 0.0 && number.fract() == 0.0 && number <= u64::MAX as f64).then_some(number as u64)
}

fn local_id(name: &str) -> Option<String> {
    let stem = name.strip_suffix(".json")?;
    let id = stem.strip_prefix("local_")?;
    uuid(id).then(|| format!("local_{id}"))
}

fn ident_component(name: Option<&std::ffi::OsStr>) -> bool {
    name.and_then(|name| name.to_str())
        .is_some_and(|name| !name.is_empty() && name.len() <= 80 && is_token(name))
}

pub fn unavailable_sessions(reason: &'static str) -> Sessions {
    Sessions {
        available: false,
        reason,
        count: 0,
        truncated: false,
        items: Vec::new(),
    }
}
