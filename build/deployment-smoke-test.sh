#!/usr/bin/env bash
set -euo pipefail

IMAGE_NAME="${IMAGE_NAME:-book-writer-chat:local}"
DEPLOY_TEST_PORT="${DEPLOY_TEST_PORT:-18080}"
CONTAINER_NAME="book-writer-chat-deployment-smoke"

cleanup() {
  docker rm -f "$CONTAINER_NAME" >/dev/null 2>&1 || true
}

trap cleanup EXIT INT TERM

docker build -f build/Dockerfile -t "$IMAGE_NAME" .

cleanup

docker run -d \
  --name "$CONTAINER_NAME" \
  -p "${DEPLOY_TEST_PORT}:8080" \
  -e FRONTEND_BASE_URL="http://127.0.0.1:${DEPLOY_TEST_PORT}" \
  "$IMAGE_NAME" >/dev/null

for _ in $(seq 1 30); do
  if curl --fail --silent "http://127.0.0.1:${DEPLOY_TEST_PORT}/api/healthz" >/dev/null; then
    break
  fi
  sleep 1
done

curl --fail --silent "http://127.0.0.1:${DEPLOY_TEST_PORT}/api/healthz" >/dev/null
curl --fail --silent "http://127.0.0.1:${DEPLOY_TEST_PORT}/readyz" >/dev/null

INDEX_HTML="$(mktemp)"
trap 'rm -f "$INDEX_HTML"; cleanup' EXIT INT TERM
curl --fail --silent "http://127.0.0.1:${DEPLOY_TEST_PORT}/reader/test-book" >"$INDEX_HTML"

if ! grep -q "__sveltekit_" "$INDEX_HTML"; then
  echo "deployment smoke test failed: expected Svelte frontend shell at /reader/test-book" >&2
  exit 1
fi
