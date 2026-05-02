#!/usr/bin/env sh
set -eu

export APP_HOST="${APP_HOST:-127.0.0.1}"
export APP_PORT="${APP_PORT:-3000}"
export BACKEND_UPSTREAM="${BACKEND_UPSTREAM:-${APP_HOST}:${APP_PORT}}"
export CADDY_SITE_ADDRESS="${CADDY_SITE_ADDRESS:-:8080}"
export APP_DATA_DIR="${APP_DATA_DIR:-/var/lib/book-writer-chat/state}"
export APP_BOOKS_ROOT="${APP_BOOKS_ROOT:-/var/lib/book-writer-chat/books-data}"
export FRONTEND_DIST_DIR="${FRONTEND_DIST_DIR:-/app/frontend/build}"
export CADDY_DATA_DIR="${CADDY_DATA_DIR:-/data}"
export CADDY_CONFIG_DIR="${CADDY_CONFIG_DIR:-/config}"

if [ -z "${FRONTEND_BASE_URL:-}" ]; then
  echo "FRONTEND_BASE_URL must be set to the public site origin, for example https://books.example.com" >&2
  exit 1
fi

mkdir -p "$APP_DATA_DIR" "$APP_BOOKS_ROOT" "$CADDY_DATA_DIR" "$CADDY_CONFIG_DIR"

terminate() {
  kill -TERM "$BACKEND_PID" "$CADDY_PID" 2>/dev/null || true
}

trap terminate INT TERM

/app/book-writer-chat &
BACKEND_PID=$!

caddy run --config /etc/caddy/Caddyfile --adapter caddyfile &
CADDY_PID=$!

while kill -0 "$BACKEND_PID" 2>/dev/null && kill -0 "$CADDY_PID" 2>/dev/null; do
  sleep 1
done

STATUS=0
if ! kill -0 "$BACKEND_PID" 2>/dev/null; then
  wait "$BACKEND_PID" || STATUS=$?
fi
if ! kill -0 "$CADDY_PID" 2>/dev/null; then
  wait "$CADDY_PID" || STATUS=$?
fi

terminate
wait "$BACKEND_PID" 2>/dev/null || true
wait "$CADDY_PID" 2>/dev/null || true

exit "$STATUS"
