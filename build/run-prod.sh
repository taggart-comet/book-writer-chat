#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if [[ -f "$SCRIPT_DIR/.env" ]]; then
  set -a
  source "$SCRIPT_DIR/.env"
  set +a
fi

export APP_ENV="${APP_ENV:-production}"
export APP_HOST="${APP_HOST:-127.0.0.1}"
export APP_PORT="${APP_PORT:-3000}"
export APP_DATA_DIR="${APP_DATA_DIR:-/var/lib/book-writer-chat/state}"
export APP_BOOKS_ROOT="${APP_BOOKS_ROOT:-/var/lib/book-writer-chat/books-data}"
export FRONTEND_DIST_DIR="${FRONTEND_DIST_DIR:-$SCRIPT_DIR/frontend/build}"
export FRONTEND_BASE_URL="${FRONTEND_BASE_URL:-}"
export BACKEND_UPSTREAM="${BACKEND_UPSTREAM:-${APP_HOST}:${APP_PORT}}"
export BACKEND_LOG_FILE="${BACKEND_LOG_FILE:-$APP_DATA_DIR/backend.log}"
export CADDY_LOG_FILE="${CADDY_LOG_FILE:-$APP_DATA_DIR/caddy.log}"
export CADDY_SITE_ADDRESS="${CADDY_SITE_ADDRESS:-}"
export XDG_DATA_HOME="${XDG_DATA_HOME:-/var/lib/book-writer-chat/caddy-data}"
export XDG_CONFIG_HOME="${XDG_CONFIG_HOME:-/var/lib/book-writer-chat/caddy-config}"

if [[ -z "$FRONTEND_BASE_URL" ]]; then
  echo "FRONTEND_BASE_URL must be set in .env to the public site origin, for example https://books.example.com" >&2
  exit 1
fi

if [[ -z "$CADDY_SITE_ADDRESS" ]]; then
  echo "CADDY_SITE_ADDRESS must be set in .env to the public hostname, for example books.example.com" >&2
  exit 1
fi

if [[ "$CADDY_SITE_ADDRESS" == :* || "$CADDY_SITE_ADDRESS" == http://* || "$CADDY_SITE_ADDRESS" == https://* ]]; then
  echo "CADDY_SITE_ADDRESS must be a bare hostname such as books.example.com so Caddy can provision HTTPS automatically" >&2
  exit 1
fi

mkdir -p "$APP_DATA_DIR" "$APP_BOOKS_ROOT" "$XDG_DATA_HOME" "$XDG_CONFIG_HOME"
touch "$BACKEND_LOG_FILE" "$CADDY_LOG_FILE"

terminate() {
  if [[ -n "${BACKEND_PID:-}" ]]; then
    kill -TERM "$BACKEND_PID" 2>/dev/null || true
  fi
  if [[ -n "${CADDY_PID:-}" ]]; then
    kill -TERM "$CADDY_PID" 2>/dev/null || true
  fi
}

trap terminate INT TERM EXIT

if command -v systemctl >/dev/null 2>&1; then
  if systemctl is-active --quiet caddy.service; then
    echo "Stopping system caddy.service so book-writer-chat can start its bundled Caddy" >&2
    systemctl stop caddy.service
  fi
fi

"$SCRIPT_DIR/book-writer-chat" >>"$BACKEND_LOG_FILE" 2>&1 &
BACKEND_PID=$!

caddy run --config "$SCRIPT_DIR/Caddyfile" --adapter caddyfile >>"$CADDY_LOG_FILE" 2>&1 &
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
