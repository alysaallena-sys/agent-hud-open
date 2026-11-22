# Agent HUD Local 0.6.11-local.1

Modified, local-only, read-only subset of Agent HUD 0.6.11.

- Upstream: <https://github.com/jazzenchen/agent-hud-open>
- License: Apache-2.0 (`LICENSE`, `NOTICE`)
- Package: `agent-hud-local`
- This document describes that package. It does not describe the rest of the tree.

The cloud environment that produced this change is Linux x86_64. It compiled and tested `agent-hud-local` there. It did not produce an arm64 macOS binary, did not run the AppKit shell, and did not apply the seatbelt profile. Those gaps are called out below. A passing test is evidence for the invocation it ran, not for a Mac process that has not been built.

## What was checked in this tree

Upstream account and session behavior was read from this source tree, not from a release binary. Release-only behavior that is not implemented here was not copied in.

| Client the user has | What this source shows | What `agent-hud-local` does |
| --- | --- | --- |
| Codex CLI | Rollouts under `sessions/YYYY/MM/DD/*.jsonl` and `archived_sessions/`. `token_count` carries `total_token_usage` and, when Codex wrote it, `rate_limits`. `auth.json` and `config.toml` can hold sign-in material. Plan refresh uses the app-server or the network. | Reads only rollout files under an opted-in home. Keeps the session id, a token-shaped model, counters, and in-range `used_percent` values Codex already wrote. Does not read `auth.json` or `config.toml`. Does not run `codex`. |
| ChatGPT desktop | The Codex desktop app is the ChatGPT app in this tree. It writes the same rollout shape and names itself with originators such as `Codex Desktop`. It keeps no `auth.json` for that engine. Live quota uses a signed-in app-server. | Same rollout reader. A file is attributed to ChatGPT desktop only when `session_meta` names a desktop originator. Counters from a file that names neither CLI nor desktop stay in `unattributed_codex` and are not assigned to either product. |
| Cursor 3.31 | `state.vscdb` holds session headers and the sign-in. Opening it is account-database access. Generation token counts for current Cursor builds are described here as hook payload fields, not as a separate metadata file. Transcripts at `agent-transcripts/<id>/<id>.jsonl` contain message text. | Does not open `state.vscdb` or any transcript. If the opted-in directory is named `agent-transcripts`, lists UUID directory names and file modification times only. Quota and token totals are `unavailable`. A Cursor application-support directory is rejected as the wrong shape and is not walked. |
| Claude Desktop | Plan quota is read from claude.ai with the app's `sessionKey` cookie. The desktop session index is `local_<uuid>.json` under `claude-code-sessions` and `local-agent-mode-sessions`. Those files are metadata. Claude Code transcripts are a different store. | Does not read cookies, `config.json`, or Claude Code `projects/` transcripts. Reads index files under 64 KiB and keeps the id, archive flag, place (`code tab` or `cowork`), and numeric `usage` only when every key in that object is an allowlisted counter. Quota is `unavailable`. |
| Claude Code CLI | Not installed for this user. | Not a source. No hooks, no `~/.claude` scan. |

No other agent integration is implemented. Upstream still has the other readers; this package does not link them.

## Capability inventory

The binary can:

- Read directories the caller passes with `--root kind=path`.
- `kind` is `codex`, `claude-desktop`, or `cursor-transcripts`.
- Open regular files that match the allowlisted relative shapes above, through one path checker that refuses symlinks, `..`, non-UTF-8 names, and denied file names.
- Keep session ids, model tokens, numeric counters, client-recorded quota windows, archive flags, and modification times.
- Print a Simple HUD transcript and one menu-bar line to stdout, or the same snapshot as JSON.
- Sleep for `--hold-ms` (at most 30 seconds) after printing. That is for tests. It is not a listener.

The binary cannot, because the code and its dependency tree do not contain the mechanism:

- Open a TCP, UDP, or Unix socket, or run an HTTP client. Direct dependencies are `serde` and `serde_json` only. Transitive crates are checked by `source_and_sandbox_files_keep_the_capability_fence`.
- Read the keychain, cookies, `auth.json`, `config.toml`, `config.json`, `state.vscdb`, or another `*.vscdb`.
- Start a child process, install a hook, MCP server, or plugin, or send a reply, approval, or auto-approval.
- Write a config, database, log, or updated session file.
- Scan `HOME`, `CODEX_HOME`, or a filesystem root. Those are not consulted. A root with no normal path component is rejected.
- Follow a symlink into or out of an opted-in root.
- Display a quota percentage it computed from token counters. A missing or out-of-range `used_percent` stays unavailable.
- Apply an OS sandbox by itself. `sandbox_applied` is always false.

Denied names include `auth.json`, `credentials.json`, `Cookies`, `state.vscdb`, `.claude.json`, `config.toml`, `hooks.json`, `config.json`, keychain-like names, and `*.pem`, `*.key`, `*.vscdb`.

The upstream desktop app, engine, MCP server, LAN listener, iCloud/phone/watch sharing, updater, and hook installer are still in this repository. Building them (`pnpm build`, `cargo build -p agent-hud`) produces the full Agent HUD, which does network and credential work. That is a different binary.

## Threat model

**Assets.** Prompts, source, tool output, cookies, tokens, API keys, and the ability to act as the user toward an agent or a provider.

**Adversary.** A modified HUD that is broader than its label: it scans the home directory, opens an account database, refreshes a quota, or keeps a copy of a prompt. Also a local process that can read whatever the HUD's process can read.

**Trust boundary.** The opted-in path is the caller's choice. The process is trusted not to leave that path except for its own executable and, when the Mac wrapper is used, the system libraries that wrapper allows. The operating system is trusted to enforce a sandbox only when that sandbox was actually applied.

**In scope for this package.** Narrow metadata from the three stores above. Fixed reasons when the metadata is not available without credentials or the network.

**Out of scope.** Protecting the user from the agent products themselves, from another process running as the same user, or from a person who builds the upstream desktop target in this same checkout.

**Security claim that is not made.** This build does not make local agent logs safe to leave on disk. It does not promise zero risk. It does not claim the Mac sandbox works until it has been run on a Mac.

## Residual risk

1. **Transient parse.** A rollout line under 256 KiB is parsed as JSON. Prompt text in that line exists in memory until the value is dropped. It is not copied into the snapshot, the HUD, or a file. Lines over 256 KiB are discarded without parsing. A test checks that a large prompt line and ordinary message fields do not appear in the snapshot. That is not a proof that the bytes never occupied RAM.
2. **Directory names.** Listing `sessions/YYYY/MM/DD` observes file names. Only `*.jsonl` in that shape are opened. Names that are not kept can still be seen by the process.
3. **Symlink races.** The checker uses `symlink_metadata` and then `File::open`. A name swapped for a symlink in between is a TOCTOU. The seatbelt profile is the control that would still confine the read to the opted-in subpath. It was not executed here.
4. **Client-recorded percentages.** `used_percent` is shown only when Codex wrote a finite value from 0 through 100, with a window length and a reset time. It can be stale. The HUD says it is client-recorded and not refreshed. It is not a live quota.
5. **Token totals.** Each non-subagent rollout contributes its latest `total_token_usage`. Subagent files are listed with their own counters and are not added, because those counters are often already inside the parent. A parent counter that excludes them will under-count. Segments of one session are summed. This can still double-count if a client repeats a cumulative total across files.
6. **Cursor ids.** UUID directory names and modification times are metadata. They are not a usage total. The transcript file is stated, not opened. A non-UUID name is ignored rather than displayed.
7. **Claude index.** A `usage` object with any key outside the counter allowlist is ignored, so a note cannot sit beside a number and be partially copied. A title field is not copied. The file is still parsed when it is at most 64 KiB.
8. **Same-user readers.** Anything the user can read, another process of that user can read. Sandboxing the HUD does not sandbox Codex, Cursor, or Claude Desktop.
9. **Full tree remains.** Someone can build upstream Agent HUD from this fork. The label on `agent-hud-local` does not follow that binary.
10. **Seatbelt profile is untested.** `(deny network*)` and `(deny file-write*)` are the intended boundary, including children if the profile is applied to the process. The mach lookups were chosen so a console Rust binary might start. They may be too tight to launch, or a future edit could add a lookup that is broader than logging. Do not remove the network deny to force a launch. `sandbox-exec` is a legacy tool; an App Sandbox entitlement on a real app bundle is the boundary for a bundled app, and that bundle was not built.
11. **Entitlements are unapplied.** `entitlements.plist` enables the App Sandbox and user-selected read-only files, and it does not grant network client, network server, keychain, Apple events, or temporary file exceptions. Entitlements do nothing until a signed app bundle uses them. This change does not sign a bundle.
12. **Simple HUD UI is uncompiled.** `src/apps/local-hud/macos/SimpleHud.swift` is an AppKit status item and a non-activating panel placed on `visibleFrame` (menu bar and notch already inset). It reads a local text file. It was not compiled, so it has not been shown on an external display.
13. **Linux network test is not a Mac result.** `the_binary_has_no_socket_inside_a_network_namespace` runs the binary under `unshare --net` and checks that process's TCP/UDP tables and file descriptors. That covers that invocation on Linux. It does not execute unbuilt branches, and it says nothing about macOS.

## Tests

`cargo test -p agent-hud-local` covers:

- Recorded Codex and ChatGPT windows stay on the product that the file names. Unnamed files are not merged into either. Inherited counters and out-of-range percents are dropped. No sentinel from prompts, instructions, tool output, `auth.json`, cookies, `config.toml`, `state.vscdb`, Claude config, or Claude Code transcripts appears in the HUD or JSON.
- Token counters without `rate_limits` do not produce a `%` in the HUD.
- No `--root` does not read `HOME`, `USERPROFILE`, or `CODEX_HOME`.
- `collect` and the binary do not change file bytes under the fixture or a sibling canary.
- A Cursor folder that is not named `agent-transcripts` is not walked.
- A filesystem root is rejected.
- The 65th oldest rollout is not summed.
- An oversized prompt line is not kept, and a later counter still is.
- The crate source has no socket, process, keychain, or SQLite API, and the lockfile closure of `agent-hud-local` does not contain `reqwest`, `hyper`, `tokio`, `rusqlite`, `tauri`, `rustls`, or `objc2`.
- The plist, seatbelt profile, wrapper, and Swift file keep the fences above.
- On Unix, a symlink root and a symlinked day directory are not followed.
- On Linux, the network-namespace invocation above.

Static fences and one Linux namespace run are not runtime proof of network isolation for the Mac build.

## Build on the Mac mini (arm64, not run here)

These commands are the intended local build. They were not run on Apple silicon or on macOS 27.

Use the Xcode Command Line Tools or Xcode that matches the machine. Do not disable Gatekeeper, and do not clear the quarantine attribute. A binary you compile yourself is a local build; these steps do not retarget a downloaded app.

```sh
rustup toolchain install 1.90.0 --profile minimal
rustup target add aarch64-apple-darwin
cargo +1.90.0 test -p agent-hud-local
cargo +1.90.0 build -p agent-hud-local --release --target aarch64-apple-darwin
```

The release binary is `target/aarch64-apple-darwin/release/agent-hud-local`.

Run it with explicit roots. Example only; replace the paths with directories you intend to open:

```sh
src/apps/local-hud/run-sandboxed.sh \
  "$PWD/target/aarch64-apple-darwin/release/agent-hud-local" \
  --once \
  --root codex=/absolute/path/to/codex-home \
  --root claude-desktop="/absolute/path/to/Library/Application Support/Claude" \
  --root cursor-transcripts=/absolute/path/to/agent-transcripts
```

The wrapper refuses to start if `sandbox-exec` is missing or the path contains sandbox metacharacters. It passes at most four roots. It was not executed in this change, so a launch failure is possible. If it fails, fix the profile without granting network.

The text HUD is the working UI. The AppKit shell is optional and unbuilt:

```sh
xcrun swiftc -O -o SimpleHud src/apps/local-hud/macos/SimpleHud.swift -framework AppKit
./agent-hud-local --once --root codex=/absolute/path/to/codex-home > /tmp/agent-hud-local.txt
./SimpleHud /tmp/agent-hud-local.txt
```

`/tmp/agent-hud-local.txt` is a copy the user asked the shell to read. The Rust binary does not write it; the redirection does. Do not point SimpleHud at a session log.

An App Sandbox bundle was not created. If you later wrap the binary in an app bundle, sign that bundle with `src/apps/local-hud/entitlements.plist` using your own signing identity. Do not use that step as a quarantine bypass, and do not add a network entitlement.

`cargo build -p agent-hud` and `pnpm build` are the upstream desktop app. They are not this build.
