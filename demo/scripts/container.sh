#!/usr/bin/env bash
set -euo pipefail
# Only the container-specific working directory is mounted here.
mkdir -p /var/lib/demo
demo_uid=${DEMO_UID:-1000}
demo_gid=${DEMO_GID:-1000}
[[ "$demo_uid" =~ ^[0-9]+$ && "$demo_gid" =~ ^[0-9]+$ ]]
chown "$demo_uid:$demo_gid" /var/lib/demo
exec gosu "$demo_uid:$demo_gid" python -B -m demo.mes.server "$@"
