//! Line parser for Codex rollouts. The JSON value is dropped at the end of
//! each line. Only allowlisted fields are copied out.

use super::{Client, RawSession};
use crate::model::{Session, Window};
use crate::text::{short_token, u64_field, uuid};
use crate::time::unix_field;
use serde_json::Value;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

const SOURCE: &str = "client-recorded rate_limits; not refreshed by this build";

struct Acc {
    id: String,
    client: Client,
    model: Option<String>,
    subagent: bool,
    started: Option<i64>,
    history_through: Option<u64>,
    observed: Option<i64>,
    tokens: Option<TokenCount>,
    windows: Vec<Window>,
}

struct TokenCount {
    input: u64,
    output: u64,
    cache_read: u64,
    cache_write: u64,
    reasoning: u64,
}

pub(super) fn parse(file: File, rel: &Path, activity: Option<i64>) -> Option<RawSession> {
    let mut acc = Acc {
        id: String::new(),
        client: Client::Unspecified,
        model: None,
        subagent: false,
        started: None,
        history_through: None,
        observed: None,
        tokens: None,
        windows: Vec::new(),
    };
    for_each_line(file, |line| absorb(&mut acc, line));
    if acc.id.is_empty() {
        acc.id = uuid_from_name(rel.file_name()?.to_str()?).unwrap_or_default();
    }
    if acc.id.is_empty() || acc.id.len() > 80 || !crate::text::is_token(&acc.id) {
        return None;
    }
    if !uuid(&acc.id) && acc.id.len() < 8 {
        return None;
    }
    if acc.client == Client::Other {
        return None;
    }
    let archived = rel
        .components()
        .next()
        .and_then(|component| component.as_os_str().to_str())
        == Some("archived_sessions");
    let tokens = acc.tokens;
    Some(RawSession {
        client: acc.client,
        windows: acc.windows,
        session: Session {
            id: acc.id,
            model: acc.model,
            archived,
            subagent: acc.subagent,
            place: None,
            observed_unix: acc.observed.or(acc.started),
            activity_unix: activity,
            input_tokens: tokens.as_ref().map(|count| count.input),
            output_tokens: tokens.as_ref().map(|count| count.output),
            cached_input_tokens: tokens.as_ref().map(|count| count.cache_read),
            cache_write_input_tokens: tokens.as_ref().map(|count| count.cache_write),
            reasoning_output_tokens: tokens.as_ref().map(|count| count.reasoning),
        },
    })
}

fn absorb(acc: &mut Acc, line: &[u8]) {
    let Ok(value) = serde_json::from_slice::<Value>(line) else {
        return;
    };
    let kind = value.get("type").and_then(Value::as_str).unwrap_or("");
    let Some(payload) = value.get("payload") else {
        return;
    };
    if kind == "session_meta" {
        note_meta(acc, payload, unix_field(value.get("timestamp")));
        return;
    }
    if kind == "turn_context" {
        if let Some(model) = short_token(payload.get("model"), 64) {
            acc.model = Some(model);
        }
        return;
    }
    if inherited(acc, &value)
        || kind != "event_msg"
        || payload.get("type").and_then(Value::as_str) != Some("token_count")
    {
        return;
    }
    let observed = unix_field(value.get("timestamp"));
    if let Some(at) = observed {
        acc.observed = Some(acc.observed.map_or(at, |kept| kept.max(at)));
    }
    if let Some(count) = token_count(payload.get("info")) {
        acc.tokens = Some(count);
    }
    let windows = windows_of(payload.get("rate_limits"), observed);
    if !windows.is_empty() {
        acc.windows = windows;
    }
}

fn note_meta(acc: &mut Acc, payload: &Value, outer_time: Option<i64>) {
    if acc.id.is_empty() {
        if let Some(id) = short_token(payload.get("id"), 80) {
            acc.id = id;
        }
    }
    acc.started = acc
        .started
        .or_else(|| unix_field(payload.get("timestamp")))
        .or(outer_time);
    acc.history_through = acc.history_through.or_else(|| {
        payload
            .get("subagent_history_start_ordinal")
            .and_then(Value::as_u64)
    });
    acc.subagent = acc.subagent || source_is_subagent(payload);
    acc.client = classify(payload.get("source"), payload.get("originator"));
}

fn inherited(acc: &Acc, value: &Value) -> bool {
    if let (Some(started), Some(at)) = (acc.started, unix_field(value.get("timestamp"))) {
        if at < started {
            return true;
        }
    }
    acc.history_through.is_some_and(|boundary| {
        value
            .get("ordinal")
            .and_then(Value::as_u64)
            .is_some_and(|ordinal| ordinal <= boundary)
    })
}

fn source_is_subagent(payload: &Value) -> bool {
    payload
        .get("source")
        .and_then(|source| source.get("subagent"))
        .is_some_and(|subagent| !subagent.is_null())
}

fn classify(source: Option<&Value>, originator: Option<&Value>) -> Client {
    match source.and_then(Value::as_str) {
        Some("cli" | "exec") => return Client::CodexCli,
        Some("vscode") => return Client::Other,
        Some("desktop") => return Client::ChatgptDesktop,
        _ => {}
    }
    match originator.and_then(Value::as_str) {
        Some("Codex Desktop" | "codex_desktop" | "codex_work_desktop") => Client::ChatgptDesktop,
        Some("codex_vscode") => Client::Other,
        Some("codex-tui" | "codex_cli_rs" | "codex-cli" | "codex_exec") => Client::CodexCli,
        Some("chatgpt" | "ChatGPT" | "chatgpt-desktop" | "ChatGPT Desktop") => {
            Client::ChatgptDesktop
        }
        _ => Client::Unspecified,
    }
}

fn token_count(info: Option<&Value>) -> Option<TokenCount> {
    let total = info?.get("total_token_usage")?;
    Some(TokenCount {
        input: u64_field(total, "input_tokens")?,
        output: u64_field(total, "output_tokens")?,
        cache_read: u64_field(total, "cached_input_tokens").unwrap_or(0),
        cache_write: u64_field(total, "cache_write_input_tokens").unwrap_or(0),
        reasoning: u64_field(total, "reasoning_output_tokens").unwrap_or(0),
    })
}

fn windows_of(limits: Option<&Value>, observed: Option<i64>) -> Vec<Window> {
    let Some(limits) = limits else {
        return Vec::new();
    };
    let plan = short_token(limits.get("plan_type"), 32);
    let mut windows = Vec::new();
    if let Some(window) = one_window("primary", limits.get("primary"), plan.clone(), observed) {
        windows.push(window);
    }
    if let Some(window) = one_window("secondary", limits.get("secondary"), plan, observed) {
        windows.push(window);
    }
    windows
}

fn one_window(
    slot: &'static str,
    value: Option<&Value>,
    plan_type: Option<String>,
    observed_unix: Option<i64>,
) -> Option<Window> {
    let value = value?;
    let used_percent = value.get("used_percent")?.as_f64()?;
    if !used_percent.is_finite() || !(0.0..=100.0).contains(&used_percent) {
        return None;
    }
    Some(Window {
        slot,
        used_percent,
        window_minutes: value.get("window_minutes")?.as_u64()?,
        resets_at_unix: value.get("resets_at")?.as_i64()?,
        plan_type,
        observed_unix,
        source: SOURCE,
    })
}

fn uuid_from_name(name: &str) -> Option<String> {
    let stem = name.strip_suffix(".jsonl").unwrap_or(name);
    let stem = stem.split('_').next().unwrap_or(stem);
    let start = stem.len().checked_sub(36)?;
    if start > 0 && !stem.is_char_boundary(start) {
        return None;
    }
    let candidate = stem.get(start..)?;
    uuid(candidate).then(|| candidate.to_owned())
}

fn for_each_line(file: File, mut on_line: impl FnMut(&[u8])) {
    let mut reader = BufReader::new(file);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        let read = reader.read_until(b'\n', &mut buf).unwrap_or(0);
        if read == 0 {
            break;
        }
        if buf.len() > crate::policy::MAX_LINE {
            if buf.last() != Some(&b'\n') {
                discard_until_newline(&mut reader);
            }
            continue;
        }
        if buf.last() == Some(&b'\n') {
            buf.pop();
        }
        if buf.last() == Some(&b'\r') {
            buf.pop();
        }
        on_line(&buf);
    }
}

fn discard_until_newline(reader: &mut BufReader<File>) {
    let mut sink = [0_u8; 8192];
    loop {
        let read = reader.read(&mut sink).unwrap_or(0);
        if read == 0 || sink[..read].contains(&b'\n') {
            break;
        }
    }
}
