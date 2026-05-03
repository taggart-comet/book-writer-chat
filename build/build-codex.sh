#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT_DIR="${OUTPUT_DIR:-$ROOT_DIR/build/bin/prod}"
IMAGE_TAG="${CODEX_DOCKER_IMAGE_TAG:-book-writer-chat-codex-builder:local}"
CONTAINER_NAME="${CODEX_DOCKER_CONTAINER_NAME:-book-writer-chat-codex-builder-extract}"
CODEX_GIT_URL="${CODEX_GIT_URL:-https://github.com/openai/codex.git}"
CODEX_GIT_REF="${CODEX_GIT_REF:-main}"
CODEX_BUILD_JOBS="${CODEX_BUILD_JOBS:-0}"

if ! command -v docker >/dev/null 2>&1; then
  echo "docker is required to build the Codex binary bundle." >&2
  exit 1
fi

mkdir -p "$OUTPUT_DIR"

docker build \
  --platform linux/amd64 \
  -f "$ROOT_DIR/build/Dockerfile.codex" \
  --build-arg CODEX_GIT_URL="$CODEX_GIT_URL" \
  --build-arg CODEX_GIT_REF="$CODEX_GIT_REF" \
  --build-arg CODEX_BUILD_JOBS="$CODEX_BUILD_JOBS" \
  -t "$IMAGE_TAG" \
  "$ROOT_DIR"

docker rm -f "$CONTAINER_NAME" >/dev/null 2>&1 || true
docker create --name "$CONTAINER_NAME" "$IMAGE_TAG" >/dev/null
docker cp "$CONTAINER_NAME:/out/codex" "$OUTPUT_DIR/codex"
docker rm -f "$CONTAINER_NAME" >/dev/null

chmod +x "$OUTPUT_DIR/codex"

printf 'Codex binary created at %s\n' "$OUTPUT_DIR/codex"
