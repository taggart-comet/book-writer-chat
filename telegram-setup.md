# Telegram Setup

This document explains how to connect a Telegram bot to this repository.

It covers two modes:

- local development with a public tunnel
- deployment on a public server

## Important Current Limitation

This repository currently accepts Telegram webhook requests at `/api/messages/telegram` and returns an internal notification payload from the backend.

It does **not** currently include a real outbound Telegram Bot API sender that posts replies back into Telegram chats automatically.

That means:

- Telegram can deliver inbound messages to this app
- the app can process `init`, `status`, and authoring commands
- the backend response includes the message text and reader URL that should be sent back to the user
- but a separate outbound delivery step is still needed if you want the bot to actually answer inside Telegram

If you only want to test inbound webhook handling and end-to-end backend behavior, the steps below are enough.

## What The App Expects

Relevant backend behavior in this repo:

- Telegram webhook endpoint: `/api/messages/telegram`
- Telegram bot username env var: `TELEGRAM_BOT_USERNAME`
- reader link base URL env var: `FRONTEND_BASE_URL`
- authoring executor path env var: `CODEX_CLI_PATH`

Telegram messages are treated as bot-directed when one of these is true:

- the message starts with `@<your_bot_username>`
- the message starts with `/bookbot`
- the message is a reply to the bot

Supported Rust-native control commands:

- `/bookbot init`
- `/bookbot status`

All other bot-directed messages become authoring requests.

## Option 1: Local Development With A Tunnel

This is the fastest way to hook Telegram to a locally running app.

### 1. Create The Telegram Bot

In Telegram:

1. Open `@BotFather`
2. Run `/newbot`
3. Choose a display name
4. Choose a username ending in `bot`
5. Save the bot token

Example username:

- `mybookwriterbot`

## 2. Start The Backend So Reader Links Use A Public URL

Telegram users cannot open `127.0.0.1` links from your laptop, so `FRONTEND_BASE_URL` must point to a public HTTPS URL.

For local development, the simplest way is to expose the backend with a tunnel and use that tunnel URL as `FRONTEND_BASE_URL`.

Start the backend like this:

```bash
TELEGRAM_BOT_USERNAME=mybookwriterbot \
FRONTEND_BASE_URL=https://replace-this-after-you-start-the-tunnel \
RUST_LOG=debug \
cargo run
```

In another terminal, start the frontend:

```bash
cd frontend
PUBLIC_BACKEND_BASE_URL=http://127.0.0.1:3000 npm run dev -- --host 0.0.0.0
```

Notes:

- the frontend still talks to the backend on `http://127.0.0.1:3000`
- Telegram only needs the backend webhook to be public
- generated reader links will use `FRONTEND_BASE_URL`

### 3. Expose The Backend With A Tunnel

Use either `ngrok` or `cloudflared`.

Example with `ngrok`:

```bash
ngrok http 3000
```

Example with `cloudflared`:

```bash
cloudflared tunnel --url http://127.0.0.1:3000
```

Copy the public HTTPS URL, for example:

```text
https://example-tunnel.ngrok-free.app
```

Then restart the backend so `FRONTEND_BASE_URL` matches that public URL:

```bash
TELEGRAM_BOT_USERNAME=mybookwriterbot \
FRONTEND_BASE_URL=https://example-tunnel.ngrok-free.app \
RUST_LOG=debug \
cargo run
```

### 4. Register The Telegram Webhook

Use your bot token from BotFather:

```bash
curl "https://api.telegram.org/bot<YOUR_BOT_TOKEN>/setWebhook?url=https://example-tunnel.ngrok-free.app/api/messages/telegram"
```

Check it:

```bash
curl "https://api.telegram.org/bot<YOUR_BOT_TOKEN>/getWebhookInfo"
```

The webhook URL should end with:

```text
/api/messages/telegram
```

### 5. Send Test Messages In Telegram

Try these messages in a chat with the bot:

- `/bookbot init`
- `/bookbot status`
- `@mybookwriterbot write an introductory chapter about habit formation`

You can also reply directly to a previous bot message with a natural-language instruction.

### 6. Watch The Local Backend Logs

Because the app uses structured tracing, backend logs are the main debugging surface:

```bash
TELEGRAM_BOT_USERNAME=mybookwriterbot \
FRONTEND_BASE_URL=https://example-tunnel.ngrok-free.app \
RUST_LOG=debug \
cargo run
```

Useful endpoints while debugging:

- `http://127.0.0.1:3000/api/healthz`
- `http://127.0.0.1:3000/api/readyz`
- `http://127.0.0.1:3000/api/metrics`

### 7. Inspect Local State

The backend stores local runtime data under:

- `books-data/`
- `data/`

Conversation-owned book workspaces are created under `books-data/` by default.

## Option 2: Public Deployment

If you want Telegram to talk to a stable public host instead of a tunnel, deploy the Docker image to a server with a public IP or public DNS.

### 1. Build The Image

```bash
make build
```

### 2. Run The Combined Container

Example:

```bash
docker run --rm \
  -p 8080:8080 \
  -e APP_ENV=production \
  -e TELEGRAM_BOT_USERNAME=mybookwriterbot \
  -e FRONTEND_BASE_URL=https://books.example.com \
  -e READER_TOKEN_SECRET=replace-with-a-real-secret \
  -v book-writer-chat-app:/var/lib/book-writer-chat \
  -v book-writer-chat-caddy-data:/data \
  -v book-writer-chat-caddy-config:/config \
  book-writer-chat:local
```

Then register the Telegram webhook:

```bash
curl "https://api.telegram.org/bot<YOUR_BOT_TOKEN>/setWebhook?url=https://books.example.com/api/messages/telegram"
```

### 3. Smoke Test The Deployed Shape Locally Before Shipping

Before deploying to a real server, verify the combined image locally:

```bash
make deployment-smoke
```

This checks:

- `/api/healthz`
- `/readyz`
- frontend routing through Caddy

## Local Debug Commands

If you want to debug without Telegram first, use the checked-in Telegram fixtures:

```bash
curl -X POST http://127.0.0.1:3000/api/messages/telegram \
  -H 'content-type: application/json' \
  --data @tests/fixtures/messenger/telegram-init.json
```

```bash
curl -X POST http://127.0.0.1:3000/api/messages/telegram \
  -H 'content-type: application/json' \
  --data @tests/fixtures/messenger/telegram-status.json
```

```bash
curl -X POST http://127.0.0.1:3000/api/messages/telegram \
  -H 'content-type: application/json' \
  --data @tests/fixtures/messenger/telegram-authoring-reply.json
```

## Common Problems

### Telegram Says The Webhook Failed

Check:

- the URL is HTTPS, not HTTP
- the URL is public, not `127.0.0.1`
- the route ends with `/api/messages/telegram`
- the backend is running

### Telegram Messages Reach The App But No Reader Link Works

Check `FRONTEND_BASE_URL`.

If it is left as `http://127.0.0.1:3000` or `http://127.0.0.1:5173`, Telegram users outside your machine will get unusable links.

For real external testing, `FRONTEND_BASE_URL` must be a public URL.

### Authoring Requests Fail Immediately

The backend launches `codex` for authoring jobs.

Check:

- `codex` is installed and on `PATH`
- or `CODEX_CLI_PATH` points to the correct executable

The control commands `init` and `status` do not require `codex`.

### The Bot Does Not Reply Inside Telegram

That is expected with the current repository state.

The backend returns a notification payload, but this repo does not yet send that payload back through Telegram Bot API automatically.

## Recommended Practical Workflow

For development:

1. verify backend behavior with the local fixture `curl` commands
2. expose port `3000` through `ngrok` or `cloudflared`
3. set Telegram webhook to `<public-url>/api/messages/telegram`
4. set `FRONTEND_BASE_URL` to the same public URL
5. test `init`, `status`, and an authoring message from Telegram

For production-like verification:

1. run `make deployment-smoke`
2. deploy the Docker image to a public host
3. point Telegram webhook at the deployed `/api/messages/telegram`
4. keep `READER_TOKEN_SECRET` non-default

