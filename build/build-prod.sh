#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT_DIR="${OUTPUT_DIR:-$ROOT_DIR/build/bin/prod}"

copy_bundle_files() {
  cp "$ROOT_DIR/build/Caddyfile" "$OUTPUT_DIR/Caddyfile"
  cp "$ROOT_DIR/build/run-prod.sh" "$OUTPUT_DIR/run-prod.sh"
  cp "$ROOT_DIR/build/.env.production.example" "$OUTPUT_DIR/.env.example"
  cp "$ROOT_DIR/build/book-writer-chat.service" "$OUTPUT_DIR/book-writer-chat.service"
  cp "$ROOT_DIR/build/INSTALL_CODEX.md" "$OUTPUT_DIR/INSTALL_CODEX.md"
  cp -R "$ROOT_DIR/frontend/build" "$OUTPUT_DIR/frontend/"

  chmod +x "$OUTPUT_DIR/book-writer-chat" "$OUTPUT_DIR/run-prod.sh"
}

build_codex_bundle() {
  OUTPUT_DIR="$OUTPUT_DIR" "$ROOT_DIR/build/build-codex.sh"
}

build_natively() {
  rm -rf "$OUTPUT_DIR"
  mkdir -p "$OUTPUT_DIR/frontend"

  (
    cd "$ROOT_DIR/frontend"
    npm ci
    PUBLIC_BACKEND_BASE_URL= npm run build
  )

  (
    cd "$ROOT_DIR"
    cargo build --release --bin book-writer-chat
  )

  cp "$ROOT_DIR/target/release/book-writer-chat" "$OUTPUT_DIR/book-writer-chat"
  build_codex_bundle
  copy_bundle_files
}

build_with_docker() {
  if ! command -v docker >/dev/null 2>&1; then
    echo "docker is required for cross-building production artifacts from non-linux/amd64 hosts." >&2
    exit 1
  fi

  rm -rf "$OUTPUT_DIR"
  mkdir -p "$OUTPUT_DIR/frontend"

  docker run --rm \
    --platform linux/amd64 \
    -v "$ROOT_DIR:/workspace" \
    -w /workspace/frontend \
    node:20-bookworm \
    bash -lc 'npm ci && PUBLIC_BACKEND_BASE_URL= npm run build'

  docker run --rm \
    --platform linux/amd64 \
    -v "$ROOT_DIR:/workspace" \
    -w /workspace \
    rust:1.87-bookworm \
    bash -lc '
      set -euo pipefail
      export PATH="/usr/local/cargo/bin:$PATH"
      cargo build --release --bin book-writer-chat
    '

  cp "$ROOT_DIR/target/release/book-writer-chat" "$OUTPUT_DIR/book-writer-chat"
  build_codex_bundle
  copy_bundle_files
}

if [[ "$(uname -s)" == "Linux" && "$(uname -m)" == "x86_64" ]]; then
  build_natively
else
  build_with_docker
fi

printf 'Production bundle created at %s\n' "$OUTPUT_DIR"
