//! Rollout parser. Conversation text is not copied. See `json_walk`.

use super::{Client, RawSession};
use crate::json_walk::{for_each_value, Hit};
use crate::model::{Session, Window};
use crate::text::{is_token, uuid};
use crate::time::unix_rfc3339_z;
use std::fs::File;
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
    for_each_value(file, |hits| apply(&mut acc, hits));
    if acc.id.is_empty() {
        acc.id = uuid_from_name(rel.file_name()?.to_str()?).unwrap_or_default();
    }
    if acc.id.is_empty() || acc.id.len() > 80 || !is_token(&acc.id) {
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

fn apply(acc: &mut Acc, hits: &[Hit]) {
    let kind = text(hits, "type");
    let at = text(hits, "timestamp").as_deref().and_then(unix_rfc3339_z);
    if kind.as_deref() == Some("session_meta") {
        if acc.id.is_empty() {
            if let Some(id) = text(hits, "payload.id").filter(|id| is_token(id) && id.len() <= 80) {
                acc.id = id;
            }
        }
        acc.started = acc.started.or_else(|| {
            text(hits, "payload.timestamp")
                .as_deref()
                .and_then(unix_rfc3339_z)
                .or(at)
        });
        acc.history_through = acc
            .history_through
            .or_else(|| u64_at(hits, "payload.subagent_history_start_ordinal"));
        acc.subagent = acc.subagent || flag(hits, "payload.source.subagent");
        acc.client = classify(
            text(hits, "payload.source").as_deref(),
            text(hits, "payload.originator").as_deref(),
        );
        return;
    }
    if kind.as_deref() == Some("turn_context") {
        if let Some(model) = text(hits, "payload.model")
            .filter(|model| !model.is_empty() && model.len() <= 64 && is_token(model))
        {
            acc.model = Some(model);
        }
        return;
    }
    if kind.as_deref() != Some("event_msg")
        || text(hits, "payload.type").as_deref() != Some("token_count")
    {
        return;
    }
    if inherited(acc, at, u64_at(hits, "ordinal")) {
        return;
    }
    if let Some(at) = at {
        acc.observed = Some(acc.observed.map_or(at, |kept| kept.max(at)));
    }
    if let Some(count) = tokens(hits) {
        acc.tokens = Some(count);
    }
    let windows = windows(hits, at);
    if !windows.is_empty() {
        acc.windows = windows;
    }
}

fn inherited(acc: &Acc, at: Option<i64>, ordinal: Option<u64>) -> bool {
    if let (Some(started), Some(at)) = (acc.started, at) {
        if at < started {
            return true;
        }
    }
    acc.history_through
        .is_some_and(|boundary| ordinal.is_some_and(|ordinal| ordinal <= boundary))
}

fn classify(source: Option<&str>, originator: Option<&str>) -> Client {
    match source {
        Some("cli" | "exec") => return Client::CodexCli,
        Some("vscode") => return Client::Other,
        Some("desktop") => return Client::ChatgptDesktop,
        _ => {}
    }
    match originator {
        Some("Codex Desktop" | "codex_desktop" | "codex_work_desktop") => Client::ChatgptDesktop,
        Some("codex_vscode") => Client::Other,
        Some("codex-tui" | "codex_cli_rs" | "codex-cli" | "codex_exec") => Client::CodexCli,
        Some("chatgpt" | "ChatGPT" | "chatgpt-desktop" | "ChatGPT Desktop") => {
            Client::ChatgptDesktop
        }
        _ => Client::Unspecified,
    }
}

fn tokens(hits: &[Hit]) -> Option<TokenCount> {
    Some(TokenCount {
        input: u64_at(hits, "payload.info.total_token_usage.input_tokens")?,
        output: u64_at(hits, "payload.info.total_token_usage.output_tokens")?,
        cache_read: u64_at(hits, "payload.info.total_token_usage.cached_input_tokens").unwrap_or(0),
        cache_write: u64_at(
            hits,
            "payload.info.total_token_usage.cache_write_input_tokens",
        )
        .unwrap_or(0),
        reasoning: u64_at(
            hits,
            "payload.info.total_token_usage.reasoning_output_tokens",
        )
        .unwrap_or(0),
    })
}

fn windows(hits: &[Hit], observed: Option<i64>) -> Vec<Window> {
    let plan = text(hits, "payload.rate_limits.plan_type")
        .filter(|plan| is_token(plan) && plan.len() <= 32);
    let mut found = Vec::new();
    if let Some(window) = one_window(
        "primary",
        "payload.rate_limits.primary",
        hits,
        plan.clone(),
        observed,
    ) {
        found.push(window);
    }
    if let Some(window) = one_window(
        "secondary",
        "payload.rate_limits.secondary",
        hits,
        plan,
        observed,
    ) {
        found.push(window);
    }
    found
}

fn one_window(
    slot: &'static str,
    prefix: &str,
    hits: &[Hit],
    plan_type: Option<String>,
    observed_unix: Option<i64>,
) -> Option<Window> {
    let used_percent = number(hits, &format!("{prefix}.used_percent"))?;
    if !used_percent.is_finite() || !(0.0..=100.0).contains(&used_percent) {
        return None;
    }
    Some(Window {
        slot,
        used_percent,
        window_minutes: u64_at(hits, &format!("{prefix}.window_minutes"))?,
        resets_at_unix: i64_at(hits, &format!("{prefix}.resets_at"))?,
        plan_type,
        observed_unix,
        source: SOURCE,
    })
}

fn text(hits: &[Hit], path: &str) -> Option<String> {
    hits.iter()
        .find(|hit| hit.path == path)
        .and_then(|hit| hit.text.clone())
}

fn number(hits: &[Hit], path: &str) -> Option<f64> {
    hits.iter()
        .find(|hit| hit.path == path)
        .and_then(|hit| hit.number)
}

fn u64_at(hits: &[Hit], path: &str) -> Option<u64> {
    let number = number(hits, path)?;
    (number >= 0.0 && number.fract() == 0.0 && number <= u64::MAX as f64).then_some(number as u64)
}

fn i64_at(hits: &[Hit], path: &str) -> Option<i64> {
    let number = number(hits, path)?;
    (number.fract() == 0.0 && number <= i64::MAX as f64 && number >= i64::MIN as f64)
        .then_some(number as i64)
}

fn flag(hits: &[Hit], path: &str) -> bool {
    hits.iter()
        .any(|hit| hit.path == path && hit.flag == Some(true))
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
