//! Codex and ChatGPT desktop rollouts. Only session ids, models, counters,
//! and client-recorded rate limits are kept. Message text is parsed only long
//! enough to ignore it.

mod codex_parse;

use crate::model::{
    empty_row, listed_sessions, reasons, Group, Quota, Row, Session, Tokens, Window,
};
use crate::policy::{Access, FILE_CAP};
use crate::text::component_name;
use codex_parse::parse;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Client {
    CodexCli,
    ChatgptDesktop,
    Unspecified,
    Other,
}

#[derive(Clone, Debug)]
struct RawSession {
    client: Client,
    session: Session,
    windows: Vec<Window>,
}

#[derive(Default)]
pub struct Scan {
    sessions: Vec<RawSession>,
    truncated: bool,
}

pub fn scan(access: &mut Access, root: &Path) -> Scan {
    let (files, truncated) = rollout_files(access, root);
    let mut sessions = Vec::new();
    for rel in files {
        if let Some(raw) = read_rollout(access, root, &rel) {
            sessions.push(raw);
        }
    }
    Scan {
        sessions,
        truncated,
    }
}

pub fn combine(scans: &[Scan], saw_root: bool) -> (Row, Row, Group) {
    if !saw_root {
        return (
            empty_row("Codex CLI", reasons::NO_PATH),
            empty_row("ChatGPT desktop", reasons::NO_PATH),
            empty_group(reasons::NO_PATH),
        );
    }
    let mut sessions = Vec::new();
    let mut truncated = false;
    for scan in scans {
        truncated |= scan.truncated;
        sessions.extend(scan.sessions.iter().cloned());
    }
    sessions.sort_by(|left, right| {
        right
            .session
            .observed_unix
            .cmp(&left.session.observed_unix)
            .then_with(|| left.session.id.cmp(&right.session.id))
    });
    let cli = row_for("Codex CLI", Client::CodexCli, &sessions, truncated);
    let desktop = row_for(
        "ChatGPT desktop",
        Client::ChatgptDesktop,
        &sessions,
        truncated,
    );
    let unattributed = group_for(Client::Unspecified, &sessions, truncated);
    (cli, desktop, unattributed)
}

fn row_for(product: &'static str, client: Client, sessions: &[RawSession], truncated: bool) -> Row {
    let mine: Vec<&RawSession> = sessions.iter().filter(|raw| raw.client == client).collect();
    finish_row(product, &mine, truncated)
}

fn group_for(client: Client, sessions: &[RawSession], truncated: bool) -> Group {
    let mine: Vec<&RawSession> = sessions.iter().filter(|raw| raw.client == client).collect();
    let row = finish_row("Codex client unnamed", &mine, truncated);
    Group {
        count: row.sessions.count,
        truncated: row.sessions.truncated,
        quota: row.quota,
        tokens: row.tokens,
        items: row.sessions.items,
    }
}

fn finish_row(product: &'static str, mine: &[&RawSession], truncated: bool) -> Row {
    let quota = newest_windows(mine);
    let tokens = sum_tokens(mine);
    let mut items: Vec<Session> = mine.iter().map(|raw| raw.session.clone()).collect();
    let item_truncated = items.len() > crate::policy::DISPLAY_CAP;
    items.truncate(crate::policy::DISPLAY_CAP);
    Row {
        product,
        quota,
        tokens,
        sessions: listed_sessions(items, truncated || item_truncated),
    }
}

fn newest_windows(mine: &[&RawSession]) -> Quota {
    let mut best: Option<&RawSession> = None;
    for raw in mine {
        if raw.windows.is_empty() {
            continue;
        }
        let replace = best.is_none_or(|kept| window_time(raw) >= window_time(kept));
        if replace {
            best = Some(raw);
        }
    }
    match best {
        Some(raw) => Quota::ClientRecorded {
            windows: raw.windows.clone(),
        },
        None => Quota::Unavailable {
            reason: reasons::CODEX_QUOTA,
        },
    }
}

fn window_time(raw: &RawSession) -> i64 {
    raw.windows
        .iter()
        .filter_map(|window| window.observed_unix)
        .max()
        .unwrap_or(i64::MIN)
}

fn sum_tokens(mine: &[&RawSession]) -> Tokens {
    let mut input = 0_u64;
    let mut output = 0_u64;
    let mut cache_read = 0_u64;
    let mut cache_write = 0_u64;
    let mut reasoning = 0_u64;
    let mut sessions_with_counters = 0_u64;
    for raw in mine {
        if raw.session.subagent {
            continue;
        }
        let Some(value) = raw.session.input_tokens else {
            continue;
        };
        sessions_with_counters += 1;
        input = input.saturating_add(value);
        output = output.saturating_add(raw.session.output_tokens.unwrap_or(0));
        cache_read = cache_read.saturating_add(raw.session.cached_input_tokens.unwrap_or(0));
        cache_write = cache_write.saturating_add(raw.session.cache_write_input_tokens.unwrap_or(0));
        reasoning = reasoning.saturating_add(raw.session.reasoning_output_tokens.unwrap_or(0));
    }
    if sessions_with_counters == 0 {
        return Tokens::Unavailable {
            reason: reasons::CODEX_TOKENS,
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

fn empty_group(reason: &'static str) -> Group {
    Group {
        count: 0,
        truncated: false,
        quota: Quota::Unavailable { reason },
        tokens: Tokens::Unavailable { reason },
        items: Vec::new(),
    }
}

fn rollout_files(access: &mut Access, root: &Path) -> (Vec<PathBuf>, bool) {
    let mut found = Vec::new();
    for top in ["sessions", "archived_sessions"] {
        if !access
            .stat(root, Path::new(top))
            .is_some_and(|stat| stat.is_dir)
        {
            continue;
        }
        for year in access.list(root, Path::new(top)) {
            if !component_name(year.file_name(), 4) {
                continue;
            }
            for month in access.list(root, &year) {
                if !component_name(month.file_name(), 2) {
                    continue;
                }
                for day in access.list(root, &month) {
                    if !component_name(day.file_name(), 2) {
                        continue;
                    }
                    for file in access.list(root, &day) {
                        if file.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
                            found.push(file);
                        }
                    }
                }
            }
        }
    }
    let truncated = found.len() > FILE_CAP;
    found.sort_by_key(|rel| {
        std::cmp::Reverse(
            access
                .stat(root, rel)
                .and_then(|stat| stat.modified_unix)
                .unwrap_or(0),
        )
    });
    found.truncate(FILE_CAP);
    (found, truncated)
}

fn read_rollout(access: &mut Access, root: &Path, rel: &Path) -> Option<RawSession> {
    let file = access.open(root, rel)?;
    let activity = access.stat(root, rel).and_then(|stat| stat.modified_unix);
    parse(file, rel, activity)
}
