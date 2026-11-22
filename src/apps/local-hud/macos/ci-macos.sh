#!/bin/sh
# Build and test agent-hud-local on a GitHub-hosted macOS runner.
# This is not the user's Mac mini and not macOS 27. A missing sandbox tool
# fails the job. This script does not change Gatekeeper.
set -eu

root=$(CDPATH= cd -- "$(dirname "$0")/../../../.." && pwd)
cd "$root"

echo "runner_os=$(sw_vers -productVersion)"
echo "runner_build=$(sw_vers -buildVersion)"
echo "runner_arch=$(uname -m)"
if [ "$(uname -m)" != "arm64" ]; then
  echo "This job expected an arm64 macOS runner and found $(uname -m)."
  exit 1
fi
if ! command -v sandbox-exec >/dev/null 2>&1; then
  echo "sandbox-exec is missing. The sandbox negative test was not run."
  exit 1
fi

if ! rustup toolchain list | grep -q '^1.90.0'; then
  rustup toolchain install 1.90.0 --profile minimal
fi

cargo +1.90.0 test -p agent-hud-local --locked
cargo +1.90.0 build -p agent-hud-local --release --locked
bin=$root/target/release/agent-hud-local
file "$bin" | tee "$root/target/agent-hud-local-file.txt"
file "$bin" | grep -q 'arm64'

work=$(mktemp -d)
inside=$work/inside
outside=$work/outside-secret.txt
mkdir -p "$inside/sessions/2026/11/18"
printf 'secret-sentinel\n' > "$outside"
printf 'readable\n' > "$inside/note.txt"
id=019a0000-0000-7000-8000-0000000000aa
cat > "$inside/sessions/2026/11/18/rollout-2026-11-18T01-00-00-$id.jsonl" <<EOF
{"timestamp":"2026-11-18T01:00:00Z","type":"session_meta","payload":{"id":"$id","timestamp":"2026-11-18T01:00:00Z","source":"cli","originator":"codex_cli_rs","instructions":"prompt-sentinel"}}
{"timestamp":"2026-11-18T01:05:00Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":15,"output_tokens":4}},"rate_limits":{"plan_type":"pro","primary":{"used_percent":42.5,"window_minutes":10080,"resets_at":1792000000}}}}
EOF

"$bin" --sandbox-self-test "$inside" "$outside" | tee "$work/sandbox-self-test.txt"
grep -q 'sandbox_applied=true' "$work/sandbox-self-test.txt"
grep -q 'outside_read=denied' "$work/sandbox-self-test.txt"
grep -q 'write=denied' "$work/sandbox-self-test.txt"
grep -q 'inside_read=ok' "$work/sandbox-self-test.txt"

"$bin" --once --allow-mixed-rollouts --root "codex=$inside" | tee "$work/hud.txt"
grep -q 'Agent HUD Local' "$work/hud.txt"
grep -q '42.5%' "$work/hud.txt"
if grep -q 'secret-sentinel' "$work/hud.txt" || grep -q 'prompt-sentinel' "$work/hud.txt"; then
  echo "HUD text contained a fixture sentinel"
  exit 1
fi

echo "swift: $(xcrun swiftc --version | head -1)"
target=arm64-apple-macos14.0
echo "compiling SimpleHud for $target"
xcrun swiftc -O -target "$target" -o "$work/SimpleHud" "$root/src/apps/local-hud/macos/SimpleHud.swift" -framework AppKit
echo "compiling SandboxProbe for $target"
xcrun clang -O2 -target "$target" -o "$work/SandboxProbe" "$root/src/apps/local-hud/macos/SandboxProbe.c"
echo "SimpleHud file type:"
file "$work/SimpleHud"
echo "SandboxProbe file type:"
file "$work/SandboxProbe"
file "$work/SimpleHud" | grep -q 'arm64'
file "$work/SandboxProbe" | grep -q 'arm64'

profile=$root/src/apps/local-hud/seatbelt.sb
inside_real=$(python3 -c 'import os,sys; print(os.path.realpath(sys.argv[1]))' "$inside")
outside_real=$(python3 -c 'import os,sys; print(os.path.realpath(sys.argv[1]))' "$outside")
probe=$(python3 -c 'import os,sys; print(os.path.realpath(sys.argv[1]))' "$work/SandboxProbe")
echo "inside_real=$inside_real"
echo "outside_real=$outside_real"
echo "probe=$probe"
echo "starting sandbox-exec"
set +e
sandbox-exec \
  -D "BIN=$probe" \
  -D "ROOT0=$inside_real" \
  -D "ROOT1=$inside_real" \
  -D "ROOT2=$inside_real" \
  -D "ROOT3=$inside_real" \
  -f "$profile" \
  "$probe" "$inside_real" "$outside_real" > "$work/probe.txt" 2> "$work/probe.err"
probe_status=$?
set -e
echo "sandbox-exec status=$probe_status"
echo "probe stdout:"
cat "$work/probe.txt"
echo "probe stderr:"
cat "$work/probe.err"
grep -q 'outside_read=denied' "$work/probe.txt"
grep -q 'inside_read=ok' "$work/probe.txt"
grep -q 'write=denied' "$work/probe.txt"
grep -q 'connect=denied' "$work/probe.txt"

"$work/SimpleHud" --self-test "$work/hud.txt" > "$work/ui.txt" &
ui=$!
i=0
while [ "$i" -lt 25 ]; do
  if ! kill -0 "$ui" 2>/dev/null; then
    break
  fi
  i=$((i + 1))
  sleep 1
done
if kill -0 "$ui" 2>/dev/null; then
  kill "$ui" || true
  echo "SimpleHud --self-test did not exit"
  exit 1
fi
wait "$ui"
cat "$work/ui.txt"
grep -q 'simple_hud_self_test status_item=1 panel=1' "$work/ui.txt"

app=$root/target/AgentHUDLocal.app
mkdir -p "$app/Contents/MacOS"
cp "$work/SimpleHud" "$app/Contents/MacOS/SimpleHud"
cp "$bin" "$app/Contents/MacOS/agent-hud-local"
cp "$root/src/apps/local-hud/macos/Info.plist" "$app/Contents/Info.plist"
# Ad-hoc signature for the bundle we just built. This does not disable Gatekeeper.
codesign --sign - "$app"
codesign --display --verbose=2 "$app" | tee "$work/codesign.txt"
ditto -c -k --keepParent "$app" "$root/target/agent-hud-local-arm64.zip"
cp "$work/sandbox-self-test.txt" "$work/probe.txt" "$work/ui.txt" "$work/hud.txt" "$root/target/"
echo "arm64 app archive: target/agent-hud-local-arm64.zip"
