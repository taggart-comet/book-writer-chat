#!/usr/bin/env bash
set -euo pipefail

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
  cargo run &
BACKEND_PID=$!

if command -v npm >/dev/null 2>&1; then
  (
    cd frontend
    if [[ ! -d node_modules ]]; then
      npm install
    fi
    PUBLIC_BACKEND_BASE_URL="${PUBLIC_BACKEND_BASE_URL:-http://127.0.0.1:3000}" \
      npm run dev -- --host 0.0.0.0
  ) &
  FRONTEND_PID=$!
fi

wait
