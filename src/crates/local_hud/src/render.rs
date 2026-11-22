//! Plain-text Simple HUD and the one menu-bar line. No window system.

use crate::model::{Quota, Snapshot, Tokens};

pub fn menu_bar(snapshot: &Snapshot) -> String {
    let mut parts = Vec::with_capacity(snapshot.rows.len() + 1);
    parts.push("Agent HUD Local".to_owned());
    for row in &snapshot.rows {
        parts.push(format!("{} {}", row.product, quota_brief(&row.quota)));
    }
    parts.join(" · ")
}

pub fn simple_hud(snapshot: &Snapshot) -> String {
    let mut lines = Vec::new();
    lines.push(menu_bar(snapshot));
    lines.push(format!(
        "{name} {version}",
        name = snapshot.name,
        version = snapshot.version
    ));
    lines.push(format!(
        "Modified build of Agent HUD {upstream} ({license})",
        upstream = snapshot.upstream_version,
        license = snapshot.license
    ));
    lines.push(format!("Upstream {upstream}", upstream = snapshot.upstream));
    lines.push(
        "Simple HUD for an external display with no notch. Notch mode is not in this build."
            .to_owned(),
    );
    lines.push(if snapshot.sandbox_applied {
        "This process applied an OS seatbelt before reading. It does not refresh quotas or open the network."
            .to_owned()
    } else {
        "This process did not apply an OS sandbox. It does not refresh quotas or open the network."
            .to_owned()
    });
    lines.push(String::new());
    for row in &snapshot.rows {
        lines.push(row.product.to_owned());
        lines.push(format!("  quota: {}", quota_text(&row.quota)));
        lines.push(format!("  tokens: {}", token_text(&row.tokens)));
        lines.push(format!("  sessions: {}", session_text(row)));
        for item in &row.sessions.items {
            let model = item.model.as_deref().unwrap_or("model unavailable");
            let when = item
                .observed_unix
                .or(item.activity_unix)
                .map(|unix| unix.to_string())
                .unwrap_or_else(|| "time unavailable".to_owned());
            lines.push(format!(
                "    {id} {model} archived={archived} subagent={subagent} at={when}",
                id = item.id,
                archived = item.archived,
                subagent = item.subagent,
            ));
        }
        lines.push(String::new());
    }
    if snapshot.unattributed_codex.count > 0 {
        lines.push("Codex client unnamed".to_owned());
        lines.push(
            "  session_meta did not name Codex CLI or ChatGPT desktop, so these counters are not assigned to either."
                .to_owned(),
        );
        lines.push(format!(
            "  quota: {}",
            quota_text(&snapshot.unattributed_codex.quota)
        ));
        lines.push(format!(
            "  tokens: {}",
            token_text(&snapshot.unattributed_codex.tokens)
        ));
        lines.push(format!("  sessions: {}", snapshot.unattributed_codex.count));
        lines.push(String::new());
    }
    lines.join("\n")
}

fn quota_brief(quota: &Quota) -> String {
    match quota {
        Quota::Unavailable { .. } => "quota unavailable".to_owned(),
        Quota::ClientRecorded { windows } => windows
            .iter()
            .find(|window| window.slot == "primary")
            .or_else(|| windows.first())
            .map(|window| format!("{} {}", window.slot, percent(window.used_percent)))
            .unwrap_or_else(|| "quota unavailable".to_owned()),
    }
}

fn quota_text(quota: &Quota) -> String {
    match quota {
        Quota::Unavailable { reason } => format!("unavailable ({reason})"),
        Quota::ClientRecorded { windows } => windows
            .iter()
            .map(|window| {
                format!(
                    "{slot} {percent} of a {minutes}-minute window, resets at {resets}, plan {plan}, observed {observed} ({source})",
                    slot = window.slot,
                    percent = percent(window.used_percent),
                    minutes = window.window_minutes,
                    resets = window.resets_at_unix,
                    plan = window.plan_type.as_deref().unwrap_or("unspecified"),
                    observed = window
                        .observed_unix
                        .map(|unix| unix.to_string())
                        .unwrap_or_else(|| "unspecified".to_owned()),
                    source = window.source,
                )
            })
            .collect::<Vec<_>>()
            .join("; "),
    }
}

fn token_text(tokens: &Tokens) -> String {
    match tokens {
        Tokens::Unavailable { reason } => format!("unavailable ({reason})"),
        Tokens::Recorded {
            input_tokens,
            output_tokens,
            cached_input_tokens,
            cache_write_input_tokens,
            reasoning_output_tokens,
            sessions_with_counters,
        } => format!(
            "recorded counters input={input_tokens} output={output_tokens} cache_read={cached_input_tokens} cache_write={cache_write_input_tokens} reasoning={reasoning_output_tokens} across {sessions_with_counters} non-subagent sessions; not a quota percentage"
        ),
    }
}

fn session_text(row: &crate::model::Row) -> String {
    if !row.sessions.available {
        return format!("unavailable ({})", row.sessions.reason);
    }
    format!(
        "{count} {reason}{more}",
        count = row.sessions.count,
        reason = row.sessions.reason,
        more = if row.sessions.truncated {
            "; list truncated"
        } else {
            ""
        }
    )
}

fn percent(value: f64) -> String {
    let text = format!("{value:.4}");
    let trimmed = text.trim_end_matches('0').trim_end_matches('.');
    format!("{trimmed}%")
}
