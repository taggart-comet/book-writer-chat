FROM node:20-bookworm AS frontend-builder
WORKDIR /workspace/frontend
COPY frontend/package.json ./
COPY frontend/package-lock.json ./
RUN npm ci
COPY frontend/ .
RUN npm run build

FROM rust:1.87-bookworm AS backend-builder
WORKDIR /workspace
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

FROM caddy:2.8.4 AS caddy-binary

FROM debian:bookworm-slim
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

COPY --from=backend-builder /workspace/target/release/book-writer-chat /app/book-writer-chat
COPY --from=caddy-binary /usr/bin/caddy /usr/bin/caddy
COPY --from=frontend-builder /workspace/frontend/build /app/frontend/build
COPY build/Caddyfile /etc/caddy/Caddyfile
COPY build/entrypoint.sh /app/entrypoint.sh

RUN chmod +x /app/entrypoint.sh \
    && mkdir -p /var/lib/book-writer-chat/state /var/lib/book-writer-chat/books-data /data /config

EXPOSE 8080

VOLUME ["/var/lib/book-writer-chat", "/data", "/config"]

ENTRYPOINT ["/app/entrypoint.sh"]
