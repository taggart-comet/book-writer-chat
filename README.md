# book-writer-chat

`book-writer-chat` is a book-making machine for people who would rather talk to a chat than open a repo.

It is not a Viper app.

It is a web app with a built-in messenger, a browser reader, a Rust backend, a Svelte frontend, and just enough structure to help a normal human write a book without being sentenced to "learn the toolchain first."

If you can imagine showing it to your mom without a 40-minute pre-brief, the product is moving in the right direction.

## What it is now

The current philosophy is simple:

- the built-in web messenger is the main authoring surface
- a book lives in a local workspace under `books-data/`
- each conversation is scoped to one book
- the backend launches Codex work inside that book workspace
- the reader is where you inspect the result without staring at source files like a raccoon in a server room

In short: chat to write, browser to review, filesystem to keep the work grounded in something real.

## What it is not

- not a generic chatbot shell
- not an enterprise workflow cathedral
- not "AI for synergy"
- not a Viper app, unless Viper apps also help your mom draft a book from a browser and somehow became useful

## Local development

Copy the example environment file:

```bash
cp .env.example .env
```

Start the app:

```bash
make up
```

That runs:

- the Rust backend on `127.0.0.1:3000`
- the Svelte frontend on `127.0.0.1:5173`

The frontend talks to the backend through `PUBLIC_BACKEND_BASE_URL`.

Useful commands:

```bash
make test
make frontend-check
make check
make seed-mock-book
```

## Deployment decision

We are deploying this as one Docker image on one `linux/amd64` virtual machine.

That image contains:

- the Rust backend binary
- the built Svelte frontend
- Caddy as the public edge

This is the current deployment philosophy:

- build one artifact
- copy one artifact to the VM
- run one container
- let Caddy terminate TLS and mint certificates automatically
- keep book data and Caddy state on mounted volumes

This repository treats `linux/amd64` as the production target. ARM laptops are fine for development, but the deployment image itself is built for `linux/amd64`.

Important runtime note:

- the application's authoring flow expects a working `codex` executable
- the current repo image packages the app, frontend, and Caddy, but does not yet visibly install the Codex CLI inside the runtime image
- before public authoring deployment, make sure the release image or runtime environment provides `codex` at `CODEX_CLI_PATH`

## Build the deployment artifact

Build the image:

```bash
make build IMAGE_NAME=book-writer-chat:release
```

If you want a portable artifact to send to the VM, export it:

```bash
mkdir -p dist
docker save -o dist/book-writer-chat-release.tar book-writer-chat:release
```

That `.tar` file is the thing we ship.

## Deliver the artifact to the VM

The simplest release path right now is `scp`, not a registry.

Copy the image tarball to the VM:

```bash
scp dist/book-writer-chat-release.tar your-user@your-vm:/tmp/
```

Copy the production env file too:

```bash
scp .env.production your-user@your-vm:/tmp/book-writer-chat.env
```

Later, if you want a registry-based flow, we can add one. For now, a tarball is boring, direct, and difficult to misunderstand, which is a strong quality in deployment systems.

## Production environment

The container needs these variables at minimum:

- `APP_ENV=production`
- `FRONTEND_BASE_URL=https://your-domain.example`
- `WEB_AUTH_USERNAME=...`
- `WEB_AUTH_PASSWORD=...`
- `JWT_SIGNING_SECRET=...`

Useful production defaults and overrides:

- `APP_PORT=3000`
- `APP_HOST=127.0.0.1`
- `APP_DATA_DIR=/var/lib/book-writer-chat/state`
- `APP_BOOKS_ROOT=/var/lib/book-writer-chat/books-data`
- `FRONTEND_DIST_DIR=/app/frontend/build`
- `CADDY_SITE_ADDRESS=your-domain.example`
- `CODEX_CLI_PATH=codex`
- `CODEX_CLI_ARGS=...`
- `AGENT_TIMEOUT_SECS=60`

If the deployed runtime uses a non-default Codex binary path, set `CODEX_CLI_PATH` explicitly.

Example `.env.production`:

```dotenv
APP_ENV=production
FRONTEND_BASE_URL=https://books.example.com
WEB_AUTH_USERNAME=operator
WEB_AUTH_PASSWORD=replace-me
JWT_SIGNING_SECRET=replace-me-with-a-long-random-secret
CADDY_SITE_ADDRESS=books.example.com
CODEX_CLI_PATH=codex
AGENT_TIMEOUT_SECS=120
```

## VM requirements

The VM should have:

- Docker installed
- ports `80` and `443` open to the public internet
- a DNS record pointing your domain at the VM
- a deployment plan for the Codex CLI runtime dependency if you want in-browser authoring to work on that VM

Caddy will handle certificate issuance and renewal automatically once:

- the domain resolves to the VM
- the container can bind public HTTP and HTTPS ports

No manual certificate ceremony is required unless you enjoy inventing chores.

## Run in production

Load the image on the VM:

```bash
docker load -i /tmp/book-writer-chat-release.tar
```

Create persistent directories:

```bash
mkdir -p /opt/book-writer-chat/env
mkdir -p /opt/book-writer-chat/data
mkdir -p /opt/book-writer-chat/caddy-data
mkdir -p /opt/book-writer-chat/caddy-config
mv /tmp/book-writer-chat.env /opt/book-writer-chat/env/.env.production
```

Run the container:

```bash
docker run -d \
  --name book-writer-chat \
  --restart unless-stopped \
  --env-file /opt/book-writer-chat/env/.env.production \
  -p 80:80 \
  -p 443:443 \
  -v /opt/book-writer-chat/data:/var/lib/book-writer-chat \
  -v /opt/book-writer-chat/caddy-data:/data \
  -v /opt/book-writer-chat/caddy-config:/config \
  book-writer-chat:release
```

What those mounts do:

- `/var/lib/book-writer-chat` keeps app state and `books-data/`
- `/data` keeps Caddy certificate state
- `/config` keeps Caddy config state

## Upgrade flow

For a new release:

1. Build a new `linux/amd64` image.
2. Export it with `docker save`.
3. Copy it to the VM with `scp`.
4. `docker load` it on the VM.
5. Stop and remove the old container.
6. Start a new container with the same mounts and env file.

Example:

```bash
docker rm -f book-writer-chat
docker run -d \
  --name book-writer-chat \
  --restart unless-stopped \
  --env-file /opt/book-writer-chat/env/.env.production \
  -p 80:80 \
  -p 443:443 \
  -v /opt/book-writer-chat/data:/var/lib/book-writer-chat \
  -v /opt/book-writer-chat/caddy-data:/data \
  -v /opt/book-writer-chat/caddy-config:/config \
  book-writer-chat:release
```

## Smoke check after deploy

Check container logs:

```bash
docker logs book-writer-chat --tail 100
```

Check health:

```bash
curl -f https://books.example.com/healthz
curl -f https://books.example.com/readyz
curl -f https://books.example.com/api/healthz
```

Open the app in a browser and confirm:

- login works
- you can create or open a book
- the messenger loads
- the reader route renders

## Repository layout

- `src/` contains the Rust backend
- `frontend/` contains the Svelte app
- `specs/` contains the specification-first planning docs
- `build/` contains the deployment packaging files
- `books-data/` is the default local manuscript workspace root

## Current operational stance

This thing is getting close enough to deployment that the README should stop pretending it is an abstract experiment.

The current stance is:

- author inside the built-in messenger
- review inside the browser reader
- deploy as one container behind Caddy
- ship the image tarball to the VM
- keep the process plain enough that, if needed, you can explain it to your mom without drawing Kubernetes on a napkin
