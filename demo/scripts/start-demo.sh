#!/usr/bin/env bash
set -euo pipefail

open_files=true
build=--build
mode=
for option in "$@"; do
    case "$option" in
        --resume|--reset)
            if [[ -n "$mode" ]]; then
                printf '%s\n' 'Choose only one of --resume and --reset.' >&2; exit 2
            fi
            mode=${option#--}
            ;;
        --no-open) open_files=false ;;
        --no-build) build=--no-build ;;
        --help)
            printf '%s\n' 'Usage: bash demo/scripts/start-demo.sh [--resume | --reset] [--no-open] [--no-build]' \
                'Interactive starts offer a choice; other starts resume by default.' \
                '--reset discards MES/Excel edits and all APEX plans in this demo, but keeps access tokens.'
            exit 0 ;;
        *) printf 'Unknown option: %s\n' "$option" >&2; exit 2 ;;
    esac
done

demo_root=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
cd -- "$demo_root"
command -v docker >/dev/null || { printf '%s\n' 'Install and start Docker with Linux containers first.' >&2; exit 1; }
compose() { MSYS_NO_PATHCONV=1 docker compose "$@"; }

if [[ -z "$mode" && -t 0 ]]; then
    printf '%s\n' '1. Resume the last state (default)' \
        '2. Reinitialize demo: discard MES/Excel edits and APEX plans; keep access tokens.' \
        '   Close the shared Excel workbook before choosing 2.'
    while true; do
        read -r -p 'Choose [1] (Ctrl+C to cancel): ' choice || exit 1
        case "$choice" in
            ''|1) mode=resume; break ;;
            2) mode=reset; break ;;
            *) printf '%s\n' 'Enter 1 or 2.' ;;
        esac
    done
fi
mode=${mode:-resume}

# Only the selected demo project's managed database volume may be removed.
reset_demo_state() (
    trap 'printf "%s\n" "Demo reinitialization did not finish. Some data may already be reset. Access tokens are retained; rerun with --reset to finish." >&2' ERR
    local database_container database_volume project other_container
    database_container=$(compose ps -q postgres)
    [[ "$database_container" =~ ^[a-f0-9]+$ ]]
    project=$(docker inspect --format '{{index .Config.Labels "com.docker.compose.project"}}' "$database_container")
    database_volume=$(docker inspect --format '{{range .Mounts}}{{if eq .Destination "/var/lib/postgresql/data"}}{{if eq .Type "volume"}}{{.Name}}{{end}}{{end}}{{end}}' "$database_container")
    [[ "$project" =~ ^[a-z0-9][a-z0-9_-]*$ && "$database_volume" =~ ^[a-zA-Z0-9][a-zA-Z0-9_.-]*$ ]]
    [[ "$(docker volume inspect --format '{{index .Labels "com.docker.compose.project"}}' "$database_volume")" == "$project" ]]
    [[ "$(docker volume inspect --format '{{index .Labels "com.docker.compose.volume"}}' "$database_volume")" == postgres-data ]]
    # Refuse a volume shared with another running or stopped container.
    for other_container in $(docker ps -aq --no-trunc --filter "volume=$database_volume"); do
        [[ "$other_container" == "$database_container" ]]
    done

    printf '%s\n' 'Restoring MES and the shared Excel workbook...'
    compose exec -T mes python -B -c '
import json
from urllib.error import HTTPError
from urllib.request import Request, urlopen
request = Request("http://127.0.0.1:8788/api/reset",
                  data=b"{\"confirmation\":\"RESET DEMO\"}",
                  headers={"Content-Type": "application/json"})
try:
    with urlopen(request, timeout=120) as response:
        assert json.load(response) == {"reset": True, "workbook_reset": True}
except HTTPError as error:
    raise SystemExit(json.load(error).get("error", "MES reset failed"))
'
    printf '%s\n' 'Recreating the APEX planning database; keeping access tokens...'
    compose stop apex postgres
    compose rm -f postgres
    docker volume rm "$database_volume" >/dev/null
    compose up --no-build -d --force-recreate --wait --wait-timeout 240
    trap - ERR
    printf '%s\n' 'Demo reinitialized. Existing MCP connections can keep using their tokens.'
)

# The shared directory must be readable by both container users.
(umask 022; mkdir -p -- .local/container)
compose up "$build" -d --wait --wait-timeout 240
if [[ "$mode" == reset ]]; then
    reset_demo_state
fi
workbook="$demo_root/.local/container/production-planning.xlsx"
if [[ ! -f "$workbook" ]]; then
    printf '%s\n' 'The demo started, but the shared Excel workbook is missing.' >&2
    exit 1
fi
mes_address=$(compose port mes 8788)
mes_url="http://${mes_address//$'\r'/}"
mes_container=$(compose ps -q mes)
compose_project=$(docker inspect --format '{{index .Config.Labels "com.docker.compose.project"}}' "$mes_container")
connection=$(compose exec -T apex apex-container connect)
native_workbook=$workbook
case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*) native_workbook=$(/usr/bin/cygpath -m "$workbook") ;;
esac

start_workbook_opener() (
    export APEX_DEMO_BRIDGE_DIR="$demo_root/.local/container/.desktop"
    export APEX_DEMO_WORKBOOK="$workbook"
    export APEX_DEMO_OPENER_SESSION APEX_DEMO_MES_CONTAINER
    APEX_DEMO_OPENER_SESSION=$(compose exec -T mes /opt/demo/scripts/container.sh --prepare-desktop --host-path "$native_workbook")
    APEX_DEMO_MES_CONTAINER=$(compose ps -q mes)
    [[ "$APEX_DEMO_OPENER_SESSION" =~ ^[a-f0-9]{32}$ && "$APEX_DEMO_MES_CONTAINER" =~ ^[a-f0-9]+$ ]] || exit 1
    case "$(uname -s)" in
        MINGW*|MSYS*|CYGWIN*)
            export APEX_DEMO_OPENER_SCRIPT APEX_DEMO_BASH_PATH APEX_DEMO_OPENER_LOG APEX_DEMO_OPENER_ERROR
            APEX_DEMO_BRIDGE_DIR=$(/usr/bin/cygpath -m "$APEX_DEMO_BRIDGE_DIR")
            APEX_DEMO_WORKBOOK=$native_workbook
            APEX_DEMO_OPENER_SCRIPT=$(/usr/bin/cygpath -m "$demo_root/scripts/workbook-opener.sh")
            APEX_DEMO_BASH_PATH=$(/usr/bin/cygpath -w /usr/bin/bash.exe)
            APEX_DEMO_OPENER_LOG=$(/usr/bin/cygpath -w "$demo_root/.local/desktop-opener-$APEX_DEMO_OPENER_SESSION.log")
            APEX_DEMO_OPENER_ERROR=$(/usr/bin/cygpath -w "$demo_root/.local/desktop-opener-$APEX_DEMO_OPENER_SESSION.err")
            powershell.exe -NoProfile -NonInteractive -Command \
                '$ErrorActionPreference = "Stop"; Start-Process -WindowStyle Hidden -FilePath $env:APEX_DEMO_BASH_PATH -ArgumentList @("--noprofile", "--norc", ([char]34 + $env:APEX_DEMO_OPENER_SCRIPT + [char]34)) -RedirectStandardOutput $env:APEX_DEMO_OPENER_LOG -RedirectStandardError $env:APEX_DEMO_OPENER_ERROR'
            ;;
        *) nohup "$BASH" "$demo_root/scripts/workbook-opener.sh" > "$demo_root/.local/desktop-opener-$APEX_DEMO_OPENER_SESSION.log" 2>&1 < /dev/null & ;;
    esac
)
if "$open_files"; then
    start_workbook_opener || printf '%s\n' 'Could not start the desktop opener. You can still open the shared workbook manually.' >&2
fi

# The page contains local connection credentials. It is outside the shared mount.
umask 077
page="$demo_root/.local/start.html"
temporary=$(mktemp "$demo_root/.local/start.XXXXXX")
trap 'rm -f -- "$temporary"' EXIT
printf '%s\n' "$connection" | compose exec -T mes python -B -m demo.onboarding \
    --mes-url "$mes_url" --workbook "$native_workbook" \
    --demo-directory "$demo_root" --compose-project "$compose_project" > "$temporary"
mv -f -- "$temporary" "$page"
trap - EXIT
unset connection

open_file() {
    case "$(uname -s)" in
        MINGW*|MSYS*|CYGWIN*)
            APEX_DEMO_OPEN_PATH=$(/usr/bin/cygpath -w "$1") \
                powershell.exe -NoProfile -NonInteractive -Command \
                '$ErrorActionPreference = "Stop"; Start-Process -WindowStyle Normal -FilePath $env:APEX_DEMO_OPEN_PATH'
            ;;
        Darwin*) open "$1" ;;
        *) xdg-open "$1" ;;
    esac
}

printf '\nDemo ready.\nMES: %s\nWorkbook: %s\nSetup page: %s\n' "$mes_url" "$native_workbook" "$page"
printf '%s\n' 'Save changes to this workbook. Close it before using Reset demo in the MES.'
if "$open_files"; then
    open_file "$page" || printf 'Could not open the browser. Open this setup page manually: %s\n' "$page" >&2
    open_file "$workbook" || printf 'Could not open a spreadsheet application. Open this file manually: %s\n' "$native_workbook" >&2
fi
