#!/usr/bin/env bash
set -euo pipefail

if (( $# > 1 )); then
    printf '%s\n' 'Usage: bash deploy/setup-mcp.sh [WORKSPACE]' >&2
    exit 2
fi
app_root=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
workspace=$(CDPATH= cd -- "${1:-$app_root}" && pwd -P)
executable="$app_root/target/release/apex"
case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*) executable+=.exe ;;
esac
if [[ ! -x "$executable" ]]; then
    printf '%s\n' 'Run cargo build --release or bash deploy/build.sh first.' >&2
    exit 1
fi

config_directory="$workspace/.codex"
config_path="$config_directory/config.toml"
if [[ -f "$config_path" ]] && grep -Eq "^[[:space:]]*\[[[:space:]]*['\"]?mcp_servers['\"]?[[:space:]]*\.[[:space:]]*['\"]?apex['\"]?[[:space:]]*(\]|\.)" "$config_path"; then
    printf '%s\n' 'APEX MCP is already configured in this project.'
    exit 0
fi

# Native Windows clients need drive paths rather than Git Bash mount paths.
case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*)
        # Use this shell's path converter, not another MSYS installation on PATH.
        executable=$(/usr/bin/cygpath -m "$executable")
        workspace=$(/usr/bin/cygpath -m "$workspace")
        ;;
esac

toml_string() {
    local value=$1
    value=${value//\\/\\\\}
    value=${value//\"/\\\"}
    value=${value//$'\b'/\\b}
    value=${value//$'\t'/\\t}
    value=${value//$'\n'/\\n}
    value=${value//$'\f'/\\f}
    value=${value//$'\r'/\\r}
    printf '"%s"' "$value"
}

mkdir -p -- "$config_directory"
cat >> "$config_path" <<EOF

[mcp_servers.apex]
command = $(toml_string "$executable")
args = ["mcp", "--with-viewer", "--workspace", $(toml_string "$workspace")]
cwd = $(toml_string "$workspace")
startup_timeout_sec = 20
tool_timeout_sec = 120
enabled = true
EOF
printf 'Configured APEX in %s. Restart the MCP connection or open a new trusted Codex task.\n' "$config_path"
