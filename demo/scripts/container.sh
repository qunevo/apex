#!/usr/bin/env bash
set -euo pipefail
# Only the container-specific working directory is mounted here.
mkdir -p /var/lib/demo
demo_uid=${DEMO_UID:-1000}
demo_gid=${DEMO_GID:-1000}
[[ "$demo_uid" =~ ^[0-9]+$ && "$demo_gid" =~ ^[0-9]+$ ]]
# The host helper and MES exchange files here. Repair older root-owned bridges.
[[ ! -L /var/lib/demo/.desktop ]] || { printf '%s\n' 'The desktop opener directory is unsafe.' >&2; exit 1; }
mkdir -p /var/lib/demo/.desktop
chown "$demo_uid:$demo_gid" /var/lib/demo /var/lib/demo/.desktop
if [[ "${1:-}" == --prepare-desktop ]]; then
    shift
    exec gosu "$demo_uid:$demo_gid" python -B -m demo.mes.desktop "$@"
fi
exec gosu "$demo_uid:$demo_gid" python -B -m demo.mes.server "$@"
