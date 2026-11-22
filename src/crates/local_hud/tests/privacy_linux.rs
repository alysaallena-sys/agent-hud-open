#![cfg(target_os = "linux")]

use std::fs;
use std::process::{Command, Stdio};
use std::time::Duration;

#[test]
fn the_binary_has_no_socket_inside_a_network_namespace() {
    let temp = tempfile::tempdir().unwrap();
    let id = "019a0000-0000-7000-8000-00000000000a";
    let root = temp.path().join("codex");
    let path = root.join(format!(
        "sessions/2026/11/18/rollout-2026-11-18T01-00-00-{id}.jsonl"
    ));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        &path,
        format!(
            r#"{{"timestamp":"2026-11-18T01:00:00Z","type":"session_meta","payload":{{"id":"{id}","timestamp":"2026-11-18T01:00:00Z","source":"cli"}}}}
{{"timestamp":"2026-11-18T01:05:00Z","type":"event_msg","payload":{{"type":"token_count","info":{{"total_token_usage":{{"input_tokens":2,"output_tokens":1}}}}}}}}
"#
        ),
    )
    .unwrap();
    let bin = env!("CARGO_BIN_EXE_agent-hud-local");
    let mut child = Command::new("unshare")
        .args([
            "--net",
            "--map-root-user",
            "--",
            bin,
            "--once",
            "--json",
            "--allow-mixed-rollouts",
            "--hold-ms",
            "2000",
            "--root",
            &format!("codex={}", root.display()),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("unshare --net is required for this regression");
    let stdout = child.stdout.take().unwrap();
    let reader = std::thread::spawn(move || {
        let mut line = String::new();
        std::io::BufRead::read_line(&mut std::io::BufReader::new(stdout), &mut line).unwrap();
        line
    });
    let line = reader.join().unwrap();
    assert!(line.contains("\"input_tokens\":2"), "{line}");
    assert_no_sockets(child.id());
    let status = child.wait().unwrap();
    assert!(status.success());
    // This is one invocation inside a Linux network namespace. It is not a
    // macOS sandbox run, and it does not prove a code path that was not
    // executed cannot open a socket.
    let _ = Duration::from_millis(1);
}

fn assert_no_sockets(pid: u32) {
    for name in ["tcp", "tcp6", "udp", "udp6"] {
        let text = fs::read_to_string(format!("/proc/{pid}/net/{name}"))
            .unwrap_or_else(|_| panic!("missing /proc/{pid}/net/{name}"));
        let rows = text.lines().filter(|line| !line.trim().is_empty()).count();
        assert_eq!(rows, 1, "{name} was not empty:\n{text}");
    }
    let fd = fs::read_dir(format!("/proc/{pid}/fd")).expect("process fd table");
    for entry in fd {
        let path = entry.unwrap().path();
        let target = fs::read_link(&path).unwrap_or_default();
        let rendered = target.to_string_lossy();
        assert!(!rendered.starts_with("socket:"), "open socket {}", rendered);
    }
}
