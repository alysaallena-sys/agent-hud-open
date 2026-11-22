#![cfg(unix)]

use agent_hud_local::{collect, OptIn, ReadOptions, RootKind};
use std::fs;
use std::os::unix::fs::symlink;
use std::path::PathBuf;

#[test]
fn symlink_rollouts_and_symlink_roots_are_not_followed() {
    let temp = tempfile::tempdir().unwrap();
    let outside = temp.path().join("outside");
    let root = temp.path().join("codex");
    fs::create_dir_all(outside.join("sessions/2026/11/18")).unwrap();
    fs::create_dir_all(&root).unwrap();
    let sentinel = "SYMLINK_SENTINEL_do_not_display";
    let id = "019a0000-0000-7000-8000-000000000009";
    let outside_file = outside.join(format!(
        "sessions/2026/11/18/rollout-2026-11-18T01-00-00-{id}.jsonl"
    ));
    fs::write(
        &outside_file,
        format!(
            r#"{{"timestamp":"2026-11-18T01:00:00Z","type":"session_meta","payload":{{"id":"{id}","timestamp":"2026-11-18T01:00:00Z","source":"cli","instructions":"{sentinel}"}}}}
{{"timestamp":"2026-11-18T01:05:00Z","type":"event_msg","payload":{{"type":"token_count","info":{{"total_token_usage":{{"input_tokens":77,"output_tokens":1}}}},"rate_limits":{{"primary":{{"used_percent":12.5,"window_minutes":10080,"resets_at":1792000000}}}}}}}}
"#
        ),
    )
    .unwrap();
    symlink(&outside, root.join("linked-home")).unwrap();
    let linked = collect(
        &[OptIn {
            kind: RootKind::CodexHome,
            path: root.join("linked-home"),
        }],
        &ReadOptions::default(),
    );
    assert!(linked.opened.is_empty());
    assert!(!serde_json::to_string(&linked.snapshot)
        .unwrap()
        .contains(sentinel));

    fs::create_dir_all(root.join("sessions/2026/11")).unwrap();
    symlink(
        outside.join("sessions/2026/11/18"),
        root.join("sessions/2026/11/18"),
    )
    .unwrap();
    let nested = collect(
        &[OptIn {
            kind: RootKind::CodexHome,
            path: root.clone(),
        }],
        &ReadOptions::default(),
    );
    assert!(nested.opened.is_empty());
    let encoded = serde_json::to_string(&nested.snapshot).unwrap();
    assert!(!encoded.contains(sentinel));
    assert!(!encoded.contains("12.5"));

    let mut fs_root = PathBuf::new();
    fs_root.push(std::path::Component::RootDir);
    assert!(collect(
        &[OptIn {
            kind: RootKind::CodexHome,
            path: fs_root,
        }],
        &ReadOptions::default(),
    )
    .opened
    .is_empty());
}
