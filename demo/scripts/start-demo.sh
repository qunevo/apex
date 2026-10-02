#!/usr/bin/env bash
set -euo pipefail

open_files=true
build=--build
for option in "$@"; do
    case "$option" in
        --no-open) open_files=false ;;
        --no-build) build=--no-build ;;
        --help) printf '%s\n' 'Usage: bash demo/scripts/start-demo.sh [--no-open] [--no-build]'; exit 0 ;;
        *) printf 'Unknown option: %s\n' "$option" >&2; exit 2 ;;
    esac
done

demo_root=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
cd -- "$demo_root"
command -v docker >/dev/null || { printf '%s\n' 'Install and start Docker with Linux containers first.' >&2; exit 1; }
compose() { MSYS_NO_PATHCONV=1 docker compose "$@"; }

# The shared directory must be readable by both container users.
(umask 022; mkdir -p -- .local/container)
compose up "$build" -d --wait --wait-timeout 240
workbook="$demo_root/.local/container/production-planning.xlsx"
if [[ ! -f "$workbook" ]]; then
    printf '%s\n' 'The demo started, but the shared Excel workbook is missing.' >&2
    exit 1
fi
mes_address=$(compose port mes 8788)
mes_url="http://${mes_address//$'\r'/}"
connection=$(compose exec -T apex apex-container connect)
native_workbook=$workbook
case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*) native_workbook=$(/usr/bin/cygpath -m "$workbook") ;;
esac

# The page contains local connection credentials. It is outside the shared mount.
umask 077
page="$demo_root/.local/start.html"
temporary=$(mktemp "$demo_root/.local/start.XXXXXX")
trap 'rm -f -- "$temporary"' EXIT
printf '%s\n' "$connection" | compose exec -T mes python -B -m demo.onboarding \
    --mes-url "$mes_url" --workbook "$native_workbook" > "$temporary"
mv -f -- "$temporary" "$page"
trap - EXIT
unset connection

open_file() {
    case "$(uname -s)" in
        MINGW*|MSYS*|CYGWIN*)
            APEX_DEMO_OPEN_PATH=$(/usr/bin/cygpath -w "$1") \
                powershell.exe -NoProfile -NonInteractive -Command \
                '$ErrorActionPreference = "Stop"; Start-Process -FilePath $env:APEX_DEMO_OPEN_PATH'
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
