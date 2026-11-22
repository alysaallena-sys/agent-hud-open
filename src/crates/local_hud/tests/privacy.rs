//! Privacy regressions for the local-only build. These checks run on every
//! operating system CI uses. Linux network-namespace checks live in
//! `privacy_linux.rs`. Symlink checks live in `privacy_unix.rs`.

use agent_hud_local::{
    collect, simple_hud, OptIn, Quota, ReadOptions, RootKind, Snapshot, Tokens, BUILD_LICENSE,
    BUILD_UPSTREAM, BUILD_VERSION,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

const CLI_ID: &str = "019a0000-0000-7000-8000-000000000001";
const DESK_ID: &str = "019a0000-0000-7000-8000-000000000002";
const PLAIN_ID: &str = "019a0000-0000-7000-8000-000000000003";
const ARCH_ID: &str = "019a0000-0000-7000-8000-000000000004";
const SUB_ID: &str = "019a0000-0000-7000-8000-000000000005";
const CLAUDE_ID: &str = "019a0000-0000-7000-8000-000000000006";
const CLAUDE_CLEAN: &str = "019a0000-0000-7000-8000-000000000007";
const CURSOR_ID: &str = "019a0000-0000-7000-8000-000000000008";

const SENTINELS: &[&str] = &[
    "INSTRUCTIONS_SENTINEL_do_not_display",
    "PROMPT_SENTINEL_do_not_display",
    "TOOL_SENTINEL_do_not_display",
    "SOURCE_SENTINEL_project",
    "CREDENTIAL_SENTINEL_token",
    "COOKIE_SENTINEL_session",
    "API_KEY_SENTINEL",
    "AUTHDB_SENTINEL",
    "CLAUDE_CONFIG_SENTINEL",
    "CLAUDE_CODE_SENTINEL",
    "TITLE_SENTINEL_do_not_display",
    "USAGE_NOTE_SENTINEL",
    "CURSOR_TRANSCRIPT_SENTINEL",
    "SECRET_HOME_SENTINEL",
    "CANARY_SENTINEL",
];

fn opted() -> ReadOptions {
    ReadOptions {
        allow_mixed_rollouts: true,
        sandbox_applied: false,
    }
}

fn write(path: &Path, body: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
}

fn codex_line(timestamp: &str, payload: &str) -> String {
    format!(r#"{{"timestamp":"{timestamp}","type":"event_msg","payload":{payload}}}"#)
}

fn session_meta(id: &str, timestamp: &str, tail: &str) -> String {
    format!(
        r#"{{"timestamp":"{timestamp}","type":"session_meta","payload":{{"id":"{id}","timestamp":"{timestamp}"{tail}}}}}"#
    )
}

fn rollup(lines: &[String]) -> String {
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

fn tree(root: &Path) -> Vec<(String, Vec<u8>)> {
    let mut files = Vec::new();
    walk(root, root, &mut files);
    files.sort();
    files
}

fn walk(root: &Path, dir: &Path, files: &mut Vec<(String, Vec<u8>)>) {
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            walk(root, &path, files);
        } else {
            let rel = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            files.push((rel, fs::read(&path).unwrap()));
        }
    }
}

fn fixture(root: &Path) {
    let cli = rollup(&[
        session_meta(
            CLI_ID,
            "2026-11-18T01:00:00Z",
            r#","source":"cli","originator":"codex_cli_rs","instructions":"INSTRUCTIONS_SENTINEL_do_not_display","cwd":"SOURCE_SENTINEL_project""#,
        ),
        codex_line(
            "2026-11-18T00:00:00Z",
            r#"{"type":"token_count","info":{"total_token_usage":{"input_tokens":999999,"output_tokens":1}},"rate_limits":{"plan_type":"pro","primary":{"used_percent":99.0,"window_minutes":10080,"resets_at":1}}}"#,
        ),
        codex_line(
            "2026-11-18T01:04:00Z",
            r#"{"type":"user_message","message":"PROMPT_SENTINEL_do_not_display"}"#,
        ),
        codex_line(
            "2026-11-18T01:05:00Z",
            r#"{"type":"token_count","info":{"total_token_usage":{"input_tokens":15,"output_tokens":4,"cached_input_tokens":3,"cache_write_input_tokens":2,"reasoning_output_tokens":1}},"rate_limits":{"plan_type":"pro","primary":{"used_percent":42.5,"window_minutes":10080,"resets_at":1792000000},"secondary":{"used_percent":7.25,"window_minutes":300,"resets_at":1791000000}}}"#,
        ),
        r#"{"timestamp":"2026-11-18T01:04:30Z","type":"turn_context","payload":{"model":"gpt-5.1-codex","prompt":"PROMPT_SENTINEL_do_not_display"}}"#.to_owned(),
        r#"{"timestamp":"2026-11-18T01:06:00Z","type":"response_item","payload":{"type":"function_call_output","output":"TOOL_SENTINEL_do_not_display"}}"#.to_owned(),
    ]);
    write(
        &root.join(format!(
            "sessions/2026/11/18/rollout-2026-11-18T01-00-00-{CLI_ID}.jsonl"
        )),
        &cli,
    );
    write(
        &root.join(format!(
            "sessions/2026/11/18/rollout-2026-11-18T01-00-00-{DESK_ID}.jsonl"
        )),
        &rollup(&[
            session_meta(
                DESK_ID,
                "2026-11-18T02:00:00Z",
                r#","originator":"Codex Desktop""#,
            ),
            codex_line(
                "2026-11-18T02:05:00Z",
                r#"{"type":"token_count","info":{"total_token_usage":{"input_tokens":4,"output_tokens":1}},"rate_limits":{"plan_type":"plus","primary":{"used_percent":9.0,"window_minutes":10080,"resets_at":1792000000}}}"#,
            ),
        ]),
    );
    write(
        &root.join(format!(
            "sessions/2026/11/18/rollout-2026-11-18T01-00-00-{PLAIN_ID}.jsonl"
        )),
        &rollup(&[
            session_meta(PLAIN_ID, "2026-11-18T03:00:00Z", ""),
            codex_line(
                "2026-11-18T03:05:00Z",
                r#"{"type":"token_count","info":{"total_token_usage":{"input_tokens":100,"output_tokens":1}},"rate_limits":{"plan_type":"pro","primary":{"used_percent":80.5,"window_minutes":10080,"resets_at":1792000000},"secondary":{"used_percent":100.1,"window_minutes":300,"resets_at":1791000000}}}"#,
            ),
        ]),
    );
    write(
        &root.join(format!(
            "archived_sessions/2026/11/17/rollout-2026-11-17T01-00-00-{ARCH_ID}.jsonl"
        )),
        &rollup(&[
            session_meta(
                ARCH_ID,
                "2026-11-17T01:00:00Z",
                r#","source":"exec","originator":"codex_exec""#,
            ),
            codex_line(
                "2026-11-17T01:05:00Z",
                r#"{"type":"token_count","info":{"total_token_usage":{"input_tokens":6,"output_tokens":1}}}"#,
            ),
        ]),
    );
    write(
        &root.join(format!(
            "sessions/2026/11/18/rollout-2026-11-18T01-00-00-{SUB_ID}.jsonl"
        )),
        &rollup(&[
            session_meta(
                SUB_ID,
                "2026-11-18T04:00:00Z",
                r#","source":{"subagent":{"thread_spawn":{}}},"originator":"codex_cli_rs""#,
            ),
            codex_line(
                "2026-11-18T04:05:00Z",
                r#"{"type":"token_count","info":{"total_token_usage":{"input_tokens":50,"output_tokens":1}}}"#,
            ),
        ]),
    );
    write(&root.join("auth.json"), "CREDENTIAL_SENTINEL_token 13.37");
    write(
        &root.join("config.toml"),
        "api_key = \"API_KEY_SENTINEL\"\n",
    );
    write(&root.join("Cookies"), "COOKIE_SENTINEL_session");
    write(&root.join("state.vscdb"), "AUTHDB_SENTINEL 13.37");
}

fn claude_fixture(root: &Path) {
    write(&root.join("config.json"), "CLAUDE_CONFIG_SENTINEL");
    write(&root.join("Cookies"), "COOKIE_SENTINEL_session");
    write(
        &root.join("projects/demo/session.jsonl"),
        "CLAUDE_CODE_SENTINEL PROMPT_SENTINEL_do_not_display",
    );
    write(
        &root.join(format!(
            "claude-code-sessions/org/acct/local_{CLAUDE_ID}.json"
        )),
        &format!(
            r#"{{"sessionId":"local_{CLAUDE_ID}","cliSessionId":"{CLAUDE_CLEAN}","isArchived":false,"title":"TITLE_SENTINEL_do_not_display","usage":{{"input_tokens":9,"output_tokens":9,"note":"USAGE_NOTE_SENTINEL"}}}}"#
        ),
    );
    write(
        &root.join(format!(
            "local-agent-mode-sessions/org/acct/local_{CLAUDE_CLEAN}.json"
        )),
        &format!(
            r#"{{"sessionId":"local_{CLAUDE_CLEAN}","isArchived":true,"usage":{{"input_tokens":8,"output_tokens":2,"cached_input_tokens":1}}}}"#
        ),
    );
}

fn cursor_fixture(root: &Path) {
    write(
        &root.join(format!("agent-transcripts/{CURSOR_ID}/{CURSOR_ID}.jsonl")),
        "CURSOR_TRANSCRIPT_SENTINEL {\"input_tokens\":12345,\"used_percent\":13.37}",
    );
    write(
        &root.join("agent-transcripts/state.vscdb"),
        "AUTHDB_SENTINEL 13.37",
    );
}

fn roots(base: &Path) -> Vec<OptIn> {
    vec![
        OptIn {
            kind: RootKind::CodexHome,
            path: base.join("codex"),
        },
        OptIn {
            kind: RootKind::ClaudeDesktop,
            path: base.join("Claude"),
        },
        OptIn {
            kind: RootKind::CursorTranscripts,
            path: base.join("agent-transcripts"),
        },
    ]
}

fn sample() -> (tempfile::TempDir, agent_hud_local::Report) {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().join("work");
    fixture(&base.join("codex"));
    claude_fixture(&base.join("Claude"));
    cursor_fixture(&base);
    write(&base.join("CANARY_SENTINEL.txt"), "CANARY_SENTINEL");
    let before = tree(&base);
    let report = collect(&roots(&base), &opted());
    assert_eq!(tree(&base), before, "collect wrote or rewrote a file");
    (temp, report)
}

fn product<'a>(snapshot: &'a Snapshot, name: &str) -> &'a agent_hud_local::Row {
    snapshot
        .rows
        .iter()
        .find(|row| row.product == name)
        .unwrap()
}

fn percents(quota: &Quota) -> Vec<f64> {
    match quota {
        Quota::ClientRecorded { windows } => {
            windows.iter().map(|window| window.used_percent).collect()
        }
        Quota::Unavailable { .. } => Vec::new(),
    }
}

#[test]
fn snapshot_keeps_recorded_counters_and_drops_secret_text() {
    let (_temp, report) = sample();
    let snap = &report.snapshot;
    assert!(snap.modified);
    assert!(!snap.sandbox_applied);
    assert_eq!(snap.mode, "simple");
    assert!(!snap.notch);
    assert_eq!(snap.version, BUILD_VERSION);
    assert_eq!(snap.license, BUILD_LICENSE);
    assert_eq!(snap.upstream, BUILD_UPSTREAM);

    let cli = product(snap, "Codex CLI");
    assert_eq!(percents(&cli.quota), vec![42.5, 7.25]);
    assert!(cli
        .sessions
        .items
        .iter()
        .any(|item| item.model.as_deref() == Some("gpt-5.1-codex")));
    match &cli.tokens {
        Tokens::Recorded {
            input_tokens,
            output_tokens,
            sessions_with_counters,
            ..
        } => {
            assert_eq!(*input_tokens, 21);
            assert_eq!(*output_tokens, 5);
            assert_eq!(*sessions_with_counters, 2);
        }
        other => panic!("expected recorded cli tokens, got {other:?}"),
    }
    assert!(cli
        .sessions
        .items
        .iter()
        .any(|item| item.id == ARCH_ID && item.archived));
    let sub = cli
        .sessions
        .items
        .iter()
        .find(|item| item.id == SUB_ID)
        .unwrap();
    assert!(sub.subagent);
    assert_eq!(sub.input_tokens, Some(50));

    let desktop = product(snap, "ChatGPT desktop");
    assert_eq!(percents(&desktop.quota), vec![9.0]);
    match &desktop.tokens {
        Tokens::Recorded { input_tokens, .. } => assert_eq!(*input_tokens, 4),
        other => panic!("expected desktop tokens, got {other:?}"),
    }

    assert_eq!(percents(&snap.unattributed_codex.quota), vec![80.5]);
    match &snap.unattributed_codex.tokens {
        Tokens::Recorded { input_tokens, .. } => assert_eq!(*input_tokens, 100),
        other => panic!("expected unattributed tokens, got {other:?}"),
    }

    let claude = product(snap, "Claude Desktop");
    assert!(matches!(claude.quota, Quota::Unavailable { .. }));
    match &claude.tokens {
        Tokens::Recorded {
            input_tokens,
            output_tokens,
            ..
        } => {
            assert_eq!(*input_tokens, 8);
            assert_eq!(*output_tokens, 2);
        }
        other => panic!("expected claude index counters, got {other:?}"),
    }
    assert!(claude
        .sessions
        .items
        .iter()
        .any(|item| item.archived && item.place == Some("cowork")));
    assert!(claude
        .sessions
        .items
        .iter()
        .any(|item| item.place == Some("code tab") && item.input_tokens.is_none()));

    let cursor = product(snap, "Cursor");
    assert!(matches!(cursor.quota, Quota::Unavailable { .. }));
    assert!(matches!(cursor.tokens, Tokens::Unavailable { .. }));
    assert_eq!(cursor.sessions.count, 1);
    assert_eq!(cursor.sessions.items[0].id, CURSOR_ID);
    assert!(cursor.sessions.items[0].input_tokens.is_none());

    let hud = simple_hud(snap);
    let encoded = serde_json::to_string(snap).unwrap();
    for sentinel in SENTINELS {
        assert!(!hud.contains(sentinel), "{sentinel} leaked into the hud");
        assert!(!encoded.contains(sentinel), "{sentinel} leaked into json");
    }
    for banned in ["999999", "13.37", "100.1", "12345", "61.25"] {
        assert!(!encoded.contains(banned), "{banned} leaked into json");
        assert!(!hud.contains(banned), "{banned} leaked into the hud");
    }
    assert!(hud.contains("42.5%"));
    assert!(hud.contains("not a quota percentage"));
    assert!(hud.contains("Modified build"));
    assert!(hud.contains("no notch"));
    assert!(report.opened.iter().all(|path| {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        name.ends_with(".jsonl") || name.ends_with(".json")
    }));
    for denied in [
        "auth.json",
        "config.toml",
        "Cookies",
        "state.vscdb",
        "config.json",
    ] {
        assert!(
            !report.opened.iter().any(|path| path.ends_with(denied)),
            "opened {denied}"
        );
    }
    assert!(!report
        .opened
        .iter()
        .any(|path| path.ends_with(format!("{CURSOR_ID}.jsonl"))));
}

#[test]
fn tokens_without_rate_limits_do_not_become_a_percentage() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("codex");
    write(
        &root.join(format!(
            "sessions/2026/11/18/rollout-2026-11-18T01-00-00-{CLI_ID}.jsonl"
        )),
        &rollup(&[
            session_meta(CLI_ID, "2026-11-18T01:00:00Z", r#","source":"cli""#),
            codex_line(
                "2026-11-18T01:05:00Z",
                r#"{"type":"token_count","info":{"total_token_usage":{"input_tokens":15,"output_tokens":4}}}"#,
            ),
        ]),
    );
    let report = collect(
        &[OptIn {
            kind: RootKind::CodexHome,
            path: root,
        }],
        &opted(),
    );
    let cli = product(&report.snapshot, "Codex CLI");
    assert!(matches!(cli.quota, Quota::Unavailable { .. }));
    assert!(matches!(cli.tokens, Tokens::Recorded { .. }));
    assert!(!simple_hud(&report.snapshot).contains('%'));
}

#[test]
fn no_opt_in_scans_nothing_and_prints_unavailable() {
    let report = collect(&[], &ReadOptions::default());
    for row in &report.snapshot.rows {
        assert!(matches!(row.quota, Quota::Unavailable { reason } if reason.contains("no path")));
    }
    assert!(report.opened.is_empty());
    assert!(!simple_hud(&report.snapshot).contains('%'));
}

#[test]
fn cursor_application_support_is_not_walked() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("Cursor");
    write(
        &root.join("User/globalStorage/state.vscdb"),
        "AUTHDB_SENTINEL 13.37",
    );
    let before = tree(&root);
    let report = collect(
        &[OptIn {
            kind: RootKind::CursorTranscripts,
            path: root.clone(),
        }],
        &ReadOptions::default(),
    );
    assert_eq!(tree(&root), before);
    assert!(report.opened.is_empty());
    let cursor = product(&report.snapshot, "Cursor");
    assert!(!cursor.sessions.available);
    assert!(!simple_hud(&report.snapshot).contains("13.37"));
    assert!(!simple_hud(&report.snapshot).contains("AUTHDB_SENTINEL"));
}

#[test]
fn filesystem_root_is_rejected() {
    let mut root = PathBuf::new();
    root.push(std::path::Component::RootDir);
    let report = collect(
        &[OptIn {
            kind: RootKind::CodexHome,
            path: root,
        }],
        &ReadOptions::default(),
    );
    assert_eq!(report.rejected_roots.len(), 1);
    assert!(report.opened.is_empty());
}

#[test]
fn file_cap_drops_the_oldest_rollout() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("codex");
    for index in 0..65 {
        let id = format!("019a0000-0000-7000-8000-{:012x}", index);
        let input = if index == 0 { 123_456_789 } else { 1 };
        let path = root.join(format!(
            "sessions/2026/11/18/rollout-2026-11-18T01-00-00-{id}.jsonl"
        ));
        let payload = format!(
            r#"{{"type":"token_count","info":{{"total_token_usage":{{"input_tokens":{input},"output_tokens":1}}}}}}"#
        );
        write(
            &path,
            &rollup(&[
                session_meta(&id, "2026-11-18T01:00:00Z", r#","source":"cli""#),
                codex_line("2026-11-18T01:05:00Z", &payload),
            ]),
        );
        let file = fs::File::options().write(true).open(&path).unwrap();
        file.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000 + index))
            .unwrap();
    }
    let report = collect(
        &[OptIn {
            kind: RootKind::CodexHome,
            path: root,
        }],
        &opted(),
    );
    let cli = product(&report.snapshot, "Codex CLI");
    match &cli.tokens {
        Tokens::Recorded { input_tokens, .. } => assert_eq!(*input_tokens, 64),
        other => panic!("expected capped tokens, got {other:?}"),
    }
    assert!(cli.sessions.truncated);
    let encoded = serde_json::to_string(&report.snapshot).unwrap();
    assert!(!encoded.contains("123456789"));
}

#[test]
fn oversized_prompt_line_is_not_kept() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("codex");
    let mut body = "PROMPT_SENTINEL_do_not_display ".repeat(20_000);
    body.push('\n');
    body.push_str(&rollup(&[
        session_meta(CLI_ID, "2026-11-18T01:00:00Z", r#","source":"cli""#),
        codex_line(
            "2026-11-18T01:05:00Z",
            r#"{"type":"token_count","info":{"total_token_usage":{"input_tokens":3,"output_tokens":1}}}"#,
        ),
    ]));
    write(
        &root.join(format!(
            "sessions/2026/11/18/rollout-2026-11-18T01-00-00-{CLI_ID}.jsonl"
        )),
        &body,
    );
    let report = collect(
        &[OptIn {
            kind: RootKind::CodexHome,
            path: root,
        }],
        &opted(),
    );
    let encoded = serde_json::to_string(&report.snapshot).unwrap();
    assert!(!encoded.contains("PROMPT_SENTINEL_do_not_display"));
    match &product(&report.snapshot, "Codex CLI").tokens {
        Tokens::Recorded { input_tokens, .. } => assert_eq!(*input_tokens, 3),
        other => panic!("expected the small counter, got {other:?}"),
    }
}

#[test]
fn binary_ignores_home_and_writes_nothing() {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().join("work");
    fixture(&base.join("codex"));
    let decoy = temp.path().join("decoy");
    write(
        &decoy.join(format!(
            "sessions/2026/11/18/rollout-2026-11-18T01-00-00-{CLI_ID}.jsonl"
        )),
        &rollup(&[
            session_meta(
                CLI_ID,
                "2026-11-18T01:00:00Z",
                r#","source":"cli","instructions":"SECRET_HOME_SENTINEL""#,
            ),
            codex_line(
                "2026-11-18T01:05:00Z",
                r#"{"type":"token_count","info":{"total_token_usage":{"input_tokens":1,"output_tokens":1}},"rate_limits":{"primary":{"used_percent":61.25,"window_minutes":10080,"resets_at":1792000000}}}"#,
            ),
        ]),
    );
    let before = tree(temp.path());
    let bin = env!("CARGO_BIN_EXE_agent-hud-local");
    let ignored = Command::new(bin)
        .current_dir(temp.path())
        .env("HOME", &decoy)
        .env("USERPROFILE", &decoy)
        .env("CODEX_HOME", &decoy)
        .arg("--once")
        .output()
        .unwrap();
    assert!(ignored.status.success());
    let ignored_out = String::from_utf8(ignored.stdout).unwrap();
    assert!(ignored_out.contains("no path opted in"));
    assert!(!ignored_out.contains("61.25"));
    assert!(!ignored_out.contains("SECRET_HOME_SENTINEL"));
    assert_eq!(tree(temp.path()), before);

    let ran = Command::new(bin)
        .current_dir(temp.path())
        .env("HOME", &decoy)
        .env("USERPROFILE", &decoy)
        .env("CODEX_HOME", &decoy)
        .args([
            "--once",
            "--json",
            "--allow-mixed-rollouts",
            "--root",
            &format!("codex={}", base.join("codex").display()),
        ])
        .output()
        .unwrap();
    assert!(
        ran.status.success(),
        "{}",
        String::from_utf8_lossy(&ran.stderr)
    );
    let out = String::from_utf8(ran.stdout).unwrap();
    if std::env::consts::OS == "macos" {
        assert!(out.contains("\"sandbox_applied\":true"));
    } else {
        assert!(out.contains("\"sandbox_applied\":false"));
    }
    assert!(out.contains("42.5"));
    assert!(!out.contains("61.25"));
    assert!(!out.contains("SECRET_HOME_SENTINEL"));
    assert!(!out.contains("CREDENTIAL_SENTINEL_token"));
    assert_eq!(tree(temp.path()), before);

    let missing = Command::new(bin)
        .args(["--root", "codex=missing-codex-home"])
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(2));
    let version = Command::new(bin).arg("--version").output().unwrap();
    let version = String::from_utf8(version.stdout).unwrap();
    assert!(version.contains(BUILD_VERSION));
    assert!(version.contains("Modified build"));
    assert!(version.contains(BUILD_UPSTREAM));
    assert!(version.contains(BUILD_LICENSE));
}

#[test]
fn source_and_sandbox_files_keep_the_capability_fence() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut rust = Vec::new();
    collect_rust(&manifest.join("src"), &mut rust);
    for (name, source) in &rust {
        let mut banned = vec![
            "std::net",
            "TcpStream",
            "TcpListener",
            "UdpSocket",
            "UnixListener",
            "UnixStream",
            "process::Command",
            "reqwest",
            "hyper",
            "tokio",
            "rustls",
            "native_tls",
            "security_framework",
            "keyring",
            "rusqlite",
            "URLSession",
            "WKWebView",
            "env::var",
            "var_os",
            "File::create",
            ".create(true)",
            ".write(true)",
        ];
        if name != "macos.rs" {
            banned.push("unsafe {");
        }
        for item in banned {
            assert!(!source.contains(item), "{name} contains {item}");
        }
    }
    assert!(rust.iter().any(|(name, _)| name == "macos.rs"));
    let toml = fs::read_to_string(manifest.join("Cargo.toml")).unwrap();
    let deps = toml
        .split("[dependencies]")
        .nth(1)
        .unwrap()
        .split("[dev-dependencies]")
        .next()
        .unwrap();
    assert!(deps.contains("serde"));
    assert!(deps.contains("serde_json"));
    assert!(!deps.contains("reqwest"));
    assert!(!deps.contains("rusqlite"));
    assert!(!deps.contains("tauri"));

    let lock = fs::read_to_string(manifest.join("../../../Cargo.lock")).unwrap();
    let mut seen = std::collections::BTreeSet::new();
    let mut queue = vec!["agent-hud-local".to_owned()];
    while let Some(name) = queue.pop() {
        if !seen.insert(name.clone()) {
            continue;
        }
        for dep in lock_deps(&lock, &name) {
            queue.push(dep);
        }
    }
    for banned in [
        "reqwest",
        "hyper",
        "tokio",
        "rusqlite",
        "libsqlite3-sys",
        "tauri",
        "rustls",
        "native-tls",
        "security-framework",
        "keyring",
        "objc2",
        "webkit2gtk",
    ] {
        assert!(!seen.contains(banned), "lock tree contains {banned}");
    }

    let plist = include_str!("../../../apps/local-hud/entitlements.plist");
    assert!(plist.contains("com.apple.security.app-sandbox"));
    assert!(plist.contains("com.apple.security.files.user-selected.read-only"));
    for banned in [
        "com.apple.security.network.client",
        "com.apple.security.network.server",
        "keychain-access-groups",
        "temporary-exception",
        "apple-events",
    ] {
        assert!(!plist.contains(banned), "entitlements contain {banned}");
    }
    let profile = include_str!("../../../apps/local-hud/seatbelt.sb");
    assert!(profile.contains("(deny network*)"));
    assert!(profile.contains("(deny file-write*)"));
    assert!(!profile.contains("(allow network"));
    let script = include_str!("../../../apps/local-hud/run-sandboxed.sh");
    assert!(script.contains("sandbox-exec"));
    assert!(script.contains("refusing"));
    for banned in ["xattr", "spctl", "quarantine", "csrutil"] {
        assert!(!script.contains(banned), "wrapper mentions {banned}");
    }
    let swift = include_str!("../../../apps/local-hud/macos/SimpleHud.swift");
    for banned in [
        "URLSession",
        "WKWebView",
        "NSAppleScript",
        "NSWorkspace",
        "Process(",
        "dlopen",
    ] {
        assert!(!swift.contains(banned), "swift contains {banned}");
    }
}

fn collect_rust(dir: &Path, out: &mut Vec<(String, String)>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_rust(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            out.push((name, fs::read_to_string(&path).unwrap()));
        }
    }
}

fn lock_deps(lock: &str, name: &str) -> Vec<String> {
    let mut deps = Vec::new();
    for package in lock.split("[[package]]").skip(1) {
        let mut lines = package.lines();
        let Some(found) = lines.find_map(|line| line.strip_prefix("name = ")) else {
            continue;
        };
        if found.trim_matches('"') != name {
            continue;
        }
        let Some(start) = package.find("dependencies = [") else {
            continue;
        };
        let body = &package[start..];
        let Some(end) = body.find(']') else {
            continue;
        };
        for line in body[..end].lines() {
            let Some(quoted) = line.split('"').nth(1) else {
                continue;
            };
            if quoted.is_empty() {
                continue;
            }
            deps.push(quoted.split_whitespace().next().unwrap().to_owned());
        }
    }
    deps
}
