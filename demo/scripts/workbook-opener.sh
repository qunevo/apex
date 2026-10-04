#!/usr/bin/env bash
# Internal desktop helper. The demo starter supplies the fixed file and session.
set -euo pipefail
bridge=${APEX_DEMO_BRIDGE_DIR:?}
workbook=${APEX_DEMO_WORKBOOK:?}
session=${APEX_DEMO_OPENER_SESSION:?}
container=${APEX_DEMO_MES_CONTAINER:?}
[[ "$session" =~ ^[a-f0-9]{32}$ && "$container" =~ ^[a-f0-9]+$ ]]
[[ -d "$bridge" && ! -L "$bridge" && -f "$workbook" && ! -L "$workbook" ]]

write_state() {
    local temporary="$bridge/.host-$$-$1"
    printf '%s\n' "$2" > "$temporary"
    mv -f -- "$temporary" "$bridge/$1"
}
open_workbook() {
    case "$(uname -s)" in
        MINGW*|MSYS*|CYGWIN*)
            APEX_DEMO_OPEN_PATH=$(/usr/bin/cygpath -w "$workbook") \
                powershell.exe -NoProfile -NonInteractive -Command \
                '$ErrorActionPreference = "Stop"; Start-Process -WindowStyle Normal -FilePath $env:APEX_DEMO_OPEN_PATH'
            ;;
        Darwin*) open "$workbook" ;;
        *) xdg-open "$workbook" ;;
    esac
}

last_request=
ticks=0
while [[ "$(cat "$bridge/session")" == "$session" ]]; do
    if (( ticks % 5 == 0 )); then
        [[ "$(docker inspect --format '{{.State.Running}}' "$container" 2>/dev/null)" == true ]] || break
    fi
    write_state heartbeat "$session $(date +%s)"
    if [[ -f "$bridge/request" && ! -L "$bridge/request" ]]; then
        read -r request_session request_id < "$bridge/request" || true
        if [[ "${request_session:-}" == "$session" && "${request_id:-}" =~ ^[a-f0-9]{32}$ && "$request_id" != "$last_request" ]]; then
            last_request=$request_id
            # Request contents never become a filename, shell command or argument.
            if [[ -f "$workbook" && ! -L "$workbook" ]] && open_workbook; then
                write_state response "$session $request_id launched"
            else
                write_state response "$session $request_id failed"
            fi
        fi
    fi
    ticks=$((ticks + 1))
    sleep 1
done
