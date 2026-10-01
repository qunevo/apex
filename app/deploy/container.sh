#!/usr/bin/env bash
set -euo pipefail
umask 077

state=/var/lib/apex
bootstrap=/run/apex-bootstrap
if (( $# == 0 )); then set -- serve; fi

password_file() {
    local destination=$1 temporary
    if [[ ! -s "$destination" ]]; then
        temporary=$(mktemp "${destination}.XXXXXX")
        openssl rand -hex 32 > "$temporary"
        mv -- "$temporary" "$destination"
    fi
}

case "${1:-serve}" in
    setup)
        password_file "$bootstrap/postgres-password"
        password_file "$bootstrap/app-password"
        # PostgreSQL's entrypoint reads this private volume as a different UID.
        chmod 644 "$bootstrap/postgres-password"
        printf 'postgres://apex_app:%s@postgres:5432/apex\n' \
            "$(cat "$bootstrap/app-password")" > "$state/database-url.tmp"
        mv -- "$state/database-url.tmp" "$state/database-url"
        if [[ ! -d "$state/identity" ]]; then
            identity=$(mktemp -d "$state/identity.XXXXXX")
            trap 'rm -rf -- "$identity"' EXIT
            apex-control init --auth "$identity/auth.json" \
                | sed -n '/^  agent (admin):/p; /^  display (viewer):/p' > "$identity/access.txt"
            mv -- "$identity" "$state/identity"
            trap - EXIT
        fi
        test -s "$state/identity/auth.json"
        test -s "$state/identity/access.txt"
        printf '%s\n' 'APEX initialized. Read connection tokens with: docker compose exec apex apex-container access'
        ;;
    migrate)
        export PGPASSWORD
        PGPASSWORD=$(cat "$bootstrap/postgres-password")
        export APEX_DATABASE_PASSWORD
        APEX_DATABASE_PASSWORD=$(cat "$bootstrap/app-password")
        psql --host postgres --username postgres --dbname apex --no-psqlrc --set ON_ERROR_STOP=1 <<'SQL'
\getenv app_password APEX_DATABASE_PASSWORD
SELECT format('CREATE ROLE apex_app LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS PASSWORD %L', :'app_password')
WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'apex_app')
\gexec
SQL
        export APEX_CONTROL_DATABASE_URL="postgres://postgres:${PGPASSWORD}@postgres:5432/apex"
        exec apex-control migrate --app-role apex_app
        ;;
    serve)
        export APEX_CONTROL_DATABASE_URL
        APEX_CONTROL_DATABASE_URL=${APEX_CONTROL_DATABASE_URL:-$(cat "$state/database-url")}
        shift
        exec apex-control serve "$@"
        ;;
    access)
        cat "$state/identity/access.txt"
        ;;
    *)
        exec apex-control "$@"
        ;;
esac
