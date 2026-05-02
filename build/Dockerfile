FROM --platform=linux/arm64 node:20-bookworm AS frontend-builder
WORKDIR /workspace/frontend
COPY frontend/package.json ./
COPY frontend/package-lock.json ./
RUN npm ci
COPY frontend/ .
RUN npm run build

FROM --platform=linux/arm64 rust:1.87-bookworm AS backend-builder
WORKDIR /workspace
RUN apt-get update \
    && apt-get install -y --no-install-recommends gcc gcc-x86-64-linux-gnu libc6-dev-amd64-cross \
    && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN set -eu; \
    rustup target add x86_64-unknown-linux-gnu; \
    export CC_x86_64_unknown_linux_gnu=x86_64-linux-gnu-gcc; \
    export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc; \
    cargo build --release --target x86_64-unknown-linux-gnu; \
    cp /workspace/target/x86_64-unknown-linux-gnu/release/book-writer-chat /tmp/book-writer-chat

FROM --platform=linux/amd64 caddy:2.8.4 AS caddy-binary

FROM --platform=linux/amd64 debian:bookworm-slim
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

ENV APP_ENV=production
ENV APP_HOST=127.0.0.1
ENV APP_PORT=3000
ENV FRONTEND_DIST_DIR=/app/frontend/build
ENV APP_DATA_DIR=/var/lib/book-writer-chat/state
ENV APP_BOOKS_ROOT=/var/lib/book-writer-chat/books-data
ENV CADDY_SITE_ADDRESS=:8080

COPY --from=backend-builder /tmp/book-writer-chat /app/book-writer-chat
COPY --from=caddy-binary /usr/bin/caddy /usr/bin/caddy
COPY --from=frontend-builder /workspace/frontend/build /app/frontend/build
COPY build/Caddyfile /etc/caddy/Caddyfile
COPY build/entrypoint.sh /app/entrypoint.sh

RUN chmod +x /app/entrypoint.sh \
    && mkdir -p /var/lib/book-writer-chat/state /var/lib/book-writer-chat/books-data /data /config

EXPOSE 8080

VOLUME ["/var/lib/book-writer-chat", "/data", "/config"]

ENTRYPOINT ["/app/entrypoint.sh"]
