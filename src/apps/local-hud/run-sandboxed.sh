#!/bin/sh
# Run agent-hud-local under the seatbelt profile in this directory.
# macOS only. If the sandbox cannot be applied, this script exits and does
# not start the binary. It does not change Gatekeeper.
set -eu

if [ "$(uname -s)" != "Darwin" ]; then
  echo "agent-hud-local: sandbox wrapper is macOS only; refusing to start" >&2
  exit 2
fi

if ! command -v sandbox-exec >/dev/null 2>&1; then
  echo "agent-hud-local: sandbox-exec is missing; refusing to start unsandboxed" >&2
  exit 2
fi

if [ "$#" -lt 1 ]; then
  echo "usage: run-sandboxed.sh /absolute/path/to/agent-hud-local [args]" >&2
  exit 2
fi

BIN=$1
shift

safe_path() {
  case "$1" in
    /*) ;;
    *)
      echo "agent-hud-local: refusing a path that is not absolute" >&2
      exit 2
      ;;
  esac
  case "$1" in
    *[\(\)\;\"\'\`\$\\\|\&\<\>]*)
      echo "agent-hud-local: refusing a path with sandbox metacharacters" >&2
      exit 2
      ;;
  esac
}

remember_root() {
  value=$1
  path=${value#*=}
  safe_path "$path"
  count=$((count + 1))
  if [ "$count" -gt 4 ]; then
    echo "agent-hud-local: refusing more than four opted-in roots" >&2
    exit 2
  fi
  if [ "$count" -eq 1 ]; then ROOT0=$path; fi
  if [ "$count" -eq 2 ]; then ROOT1=$path; fi
  if [ "$count" -eq 3 ]; then ROOT2=$path; fi
  if [ "$count" -eq 4 ]; then ROOT3=$path; fi
}

safe_path "$BIN"

ROOT0=/usr/lib
ROOT1=/usr/lib
ROOT2=/usr/lib
ROOT3=/usr/lib
count=0
previous=
for arg in "$@"; do
  case "$arg" in
    --root=*)
      remember_root "${arg#--root=}"
      ;;
  esac
  if [ "$previous" = "--root" ]; then
    remember_root "$arg"
  fi
  previous=$arg
done

if [ "$count" -lt 2 ]; then ROOT1=$ROOT0; fi
if [ "$count" -lt 3 ]; then ROOT2=$ROOT0; fi
if [ "$count" -lt 4 ]; then ROOT3=$ROOT0; fi

DIR=$(CDPATH= cd -- "$(dirname "$0")" && pwd)
PROFILE=$DIR/seatbelt.sb

exec sandbox-exec \
  -D "BIN=$BIN" \
  -D "ROOT0=$ROOT0" \
  -D "ROOT1=$ROOT1" \
  -D "ROOT2=$ROOT2" \
  -D "ROOT3=$ROOT3" \
  -f "$PROFILE" \
  "$BIN" "$@"
