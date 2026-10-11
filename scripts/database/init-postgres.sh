#!/usr/bin/env bash
set -euo pipefail
: "${APP_DATABASE_USER:?APP_DATABASE_USER required}"
: "${APP_DATABASE_PASSWORD:?APP_DATABASE_PASSWORD required}"
psql --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" --set=ON_ERROR_STOP=1 \
  --set=app_role="$APP_DATABASE_USER" --set=app_password="$APP_DATABASE_PASSWORD" \
  --set=app_database="$POSTGRES_DB" <<'SQL'
CREATE ROLE :"app_role" LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION PASSWORD :'app_password';
ALTER DATABASE :"app_database" OWNER TO :"app_role";
SQL
