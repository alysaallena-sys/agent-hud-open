//! Command line for the modified local-only build.
//! Paths come only from `--root`. Nothing is written.
//! On macOS the process applies a seatbelt before reading. The only `unsafe`
//! is that seatbelt call in `macos.rs`.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(target_os = "macos"))]
mod macos {
    pub fn confine(_roots: &[std::path::PathBuf]) -> Result<bool, String> {
        Ok(false)
    }

    pub fn self_test(_inside: &std::path::Path, _outside: &std::path::Path) -> Result<(), String> {
        Err("sandbox self-test runs on macOS".to_owned())
    }
}

use agent_hud_local::{
    simple_hud, OptIn, ReadOptions, RootKind, BUILD_LICENSE, BUILD_UPSTREAM, BUILD_VERSION,
};
use std::env;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::thread;
use std::time::Duration;

fn main() -> ExitCode {
    match run(env::args().skip(1)) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("agent-hud-local: {message}");
            ExitCode::from(2)
        }
    }
}

fn run(args: impl Iterator<Item = String>) -> Result<ExitCode, String> {
    let mut json = false;
    let mut hold_ms = 0_u64;
    let mut allow_mixed_rollouts = false;
    let mut self_test = None;
    let mut roots = Vec::new();
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--version" => {
                println!("{}", version_text());
                return Ok(ExitCode::SUCCESS);
            }
            "--help" | "-h" => {
                println!("{}", help_text());
                return Ok(ExitCode::SUCCESS);
            }
            "--json" => json = true,
            "--once" => {}
            "--allow-mixed-rollouts" => allow_mixed_rollouts = true,
            "--sandbox-self-test" => {
                let inside = args
                    .next()
                    .ok_or_else(|| "--sandbox-self-test needs INSIDE OUTSIDE".to_owned())?;
                let outside = args
                    .next()
                    .ok_or_else(|| "--sandbox-self-test needs INSIDE OUTSIDE".to_owned())?;
                self_test = Some((inside, outside));
            }
            "--hold-ms" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--hold-ms needs a number of milliseconds".to_owned())?;
                hold_ms = parse_hold(&value)?;
            }
            "--root" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--root needs KIND=PATH".to_owned())?;
                roots.push(parse_root(&value)?);
            }
            other if other.starts_with("--root=") => {
                roots.push(parse_root(other.trim_start_matches("--root="))?);
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if let Some((inside, outside)) = self_test {
        macos::self_test(
            std::path::Path::new(&inside),
            std::path::Path::new(&outside),
        )?;
        return Ok(ExitCode::SUCCESS);
    }
    let root_paths: Vec<std::path::PathBuf> = roots.iter().map(|root| root.path.clone()).collect();
    let sandbox_applied = macos::confine(&root_paths)?;
    let report = agent_hud_local::collect(
        &roots,
        &ReadOptions {
            allow_mixed_rollouts,
            sandbox_applied,
        },
    );
    if json {
        println!(
            "{}",
            serde_json::to_string(&report.snapshot).map_err(|_| "could not encode the snapshot")?
        );
    } else {
        println!("{}", simple_hud(&report.snapshot));
    }
    let _ = io::stdout().flush();
    if hold_ms > 0 {
        thread::sleep(Duration::from_millis(hold_ms));
    }
    if report.rejected_roots.is_empty() {
        Ok(ExitCode::SUCCESS)
    } else {
        for (path, reason) in &report.rejected_roots {
            eprintln!("opted-in path rejected ({reason}): {}", path.display());
        }
        Ok(ExitCode::from(2))
    }
}

fn parse_hold(value: &str) -> Result<u64, String> {
    let parsed: u64 = value
        .parse()
        .map_err(|_| "--hold-ms needs a number from 0 to 30000".to_owned())?;
    if parsed > 30_000 {
        return Err("--hold-ms needs a number from 0 to 30000".to_owned());
    }
    Ok(parsed)
}

fn parse_root(value: &str) -> Result<OptIn, String> {
    let (kind, path) = value
        .split_once('=')
        .ok_or_else(|| "--root needs KIND=PATH".to_owned())?;
    if path.is_empty() {
        return Err("--root needs KIND=PATH".to_owned());
    }
    let kind = match kind {
        "codex" => RootKind::CodexHome,
        "claude-desktop" => RootKind::ClaudeDesktop,
        "cursor-transcripts" => RootKind::CursorTranscripts,
        _ => {
            return Err(
                "--root kind must be codex, claude-desktop, or cursor-transcripts".to_owned(),
            )
        }
    };
    Ok(OptIn {
        kind,
        path: PathBuf::from(path),
    })
}

fn version_text() -> String {
    format!(
        "{name} {version}\nModified build of Agent HUD {upstream_version}\nUpstream {upstream}\nLicense {license}\nLocal-only and read-only. Not the upstream release.\n",
        name = agent_hud_local::BUILD_NAME,
        version = BUILD_VERSION,
        upstream_version = agent_hud_local::BUILD_UPSTREAM_VERSION,
        upstream = BUILD_UPSTREAM,
        license = BUILD_LICENSE,
    )
}

fn help_text() -> &'static str {
    "agent-hud-local --once [--json] [--allow-mixed-rollouts] [--hold-ms N] [--root KIND=PATH ...]\n\
     KIND is codex, claude-desktop, or cursor-transcripts.\n\
     Each PATH is an explicit opt-in. This build does not scan a home directory,\n\
     read credentials, refresh quotas, or open a network connection.\n\
     Codex rollout files also contain conversation text. They are not read unless\n\
     --allow-mixed-rollouts is set. The reader discards those strings and does not\n\
     store them; a 512-byte buffer can still hold a fragment.\n\
     On macOS a seatbelt is applied before the read. Nothing is written."
}
