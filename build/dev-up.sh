#!/usr/bin/env bash
set -euo pipefail

if [[ -f .env ]]; then
  set -a
  source .env
  set +a
fi

cleanup() {
  if [[ -n "${BACKEND_PID:-}" ]]; then
    kill "$BACKEND_PID" >/dev/null 2>&1 || true
  fi
  if [[ -n "${FRONTEND_PID:-}" ]]; then
    kill "$FRONTEND_PID" >/dev/null 2>&1 || true
  fi
}

trap cleanup EXIT INT TERM

FRONTEND_BASE_URL="${FRONTEND_BASE_URL:-http://127.0.0.1:5173}" \
APP_DATA_DIR="${APP_DATA_DIR:-target/dev/data}" \
WEB_AUTH_USERNAME="${WEB_AUTH_USERNAME:-operator}" \
WEB_AUTH_PASSWORD="${WEB_AUTH_PASSWORD:-secret-password}" \
JWT_SIGNING_SECRET="${JWT_SIGNING_SECRET:-dev-jwt-signing-secret}" \
  cargo run --bin book-writer-chat &
BACKEND_PID=$!

if command -v npm >/dev/null 2>&1; then
  (
    cd frontend
    if [[ ! -d node_modules ]]; then
      npm install
    fi
    PUBLIC_BACKEND_BASE_URL="${PUBLIC_BACKEND_BASE_URL:-http://127.0.0.1:3000}" \
      npm run dev -- --host 127.0.0.1 --port 5173 --strictPort
  ) &
  FRONTEND_PID=$!
fi

wait
