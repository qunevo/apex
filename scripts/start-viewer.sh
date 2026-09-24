#!/usr/bin/env bash
set -euo pipefail

port=${1:-8765}
if [[ $# -gt 1 || ! "$port" =~ ^[0-9]{1,5}$ ]] || (( 10#$port < 1 || 10#$port > 65535 )); then
    printf '%s\n' 'Usage: bash scripts/start-viewer.sh [PORT (1-65535)]' >&2
    exit 2
fi

workspace=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
executable="$workspace/target/release/apex"
case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*) executable+=.exe ;;
esac
if [[ ! -x "$executable" ]]; then
    printf '%s\n' 'Run cargo build --release or bash scripts/build.sh first.' >&2
    exit 1
fi

exec "$executable" serve --port "$((10#$port))" --workspace "$workspace"
