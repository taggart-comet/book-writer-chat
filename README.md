# book-writer-chat

Foundation workspace for the Book Writer Chat system: a Rust backend, a SvelteKit frontend, and local conversation-owned book workspaces under `books-data/`.

## Repository Layout

- `src/`: backend entrypoint, app router, and architecture-aligned module skeleton
- `frontend/`: SvelteKit reader shell and browser client
- `specs/`: product, architecture, and build-plan specs
- `build/`: local/dev and deployment-oriented scripts and container assets
- `books-data/`: local book workspaces created at runtime and ignored by Git

## Local Development

### Prerequisites

- Rust toolchain with `cargo`
- Node.js with `npm`
- Docker for `make build` and `make deployment-smoke`

### Start Both Apps

```bash
make up
```

This starts:

- backend on `http://127.0.0.1:3000`
- frontend dev server on `http://127.0.0.1:5173`

The frontend reads `PUBLIC_BACKEND_BASE_URL` and defaults to `http://127.0.0.1:3000` in the provided local workflow.
The backend sets `FRONTEND_BASE_URL=http://127.0.0.1:5173` inside `make up`, so messenger replies open the live Svelte reader during local development.

### Start Apps Separately

```bash
make backend
```

```bash
make frontend-install
make frontend
```

### Build The Deployment Image

```bash
make build
```

The deployable image is assembled from [build/Dockerfile](/Users/maksimtaisov/RustroverProjects/book-writer-chat/build/Dockerfile), runs the Rust backend and Caddy through [build/entrypoint.sh](/Users/maksimtaisov/RustroverProjects/book-writer-chat/build/entrypoint.sh), and serves the built Svelte app directly from Caddy using [build/Caddyfile](/Users/maksimtaisov/RustroverProjects/book-writer-chat/build/Caddyfile).

### Smoke Test The Combined Container

```bash
make deployment-smoke
```

This local smoke flow builds the image, starts the combined container, checks that `/api/healthz` is routed to the backend, and checks that browser routes return the built frontend through Caddy.

### Run Verification

```bash
make check
```

This runs the backend Rust tests plus the frontend typecheck and production build.

### Example Production Run

```bash
docker run --rm \
  -p 8080:8080 \
  -e FRONTEND_BASE_URL=https://books.example.com \
  -e READER_TOKEN_SECRET=replace-with-a-real-secret \
  -v book-writer-chat-app:/var/lib/book-writer-chat \
  -v book-writer-chat-caddy-data:/data \
  -v book-writer-chat-caddy-config:/config \
  book-writer-chat:local
```

## Verification

- `make test` is the canonical backend end-to-end harness. The backend integration coverage now lives in `src/app/test_support.rs` and covers messenger command handling, workspace mutation, revision persistence, signed reader links, reader API fetches, and two-conversation isolation.
- `make frontend-check` is the canonical frontend regression harness.
- `make check` runs the main backend and frontend verification suite together.
- `make up` is the canonical local browser verification path. In that workflow, signed links open the Svelte reader on `http://127.0.0.1:5173`, and the backend `/reader/:token` route also renders the latest draft directly when the frontend is not in front of it.
- `make deployment-smoke` is the canonical deployment-like verification path for the combined image and Caddy routing layer.

## Environment Notes

- Backend runtime defaults to `books-data/` for local development.
- Backend runtime defaults to `target/test/books-data/` in `APP_ENV=test`.
- The production container expects `FRONTEND_BASE_URL` to be set to the public site origin and `READER_TOKEN_SECRET` to be set to a non-default secret.
- The production container serves the frontend from Caddy and keeps the backend on the internal `APP_HOST` and `APP_PORT` pair, which defaults to `127.0.0.1:3000`.
- The production image declares `/var/lib/book-writer-chat` for application state and book workspaces, plus `/data` and `/config` for Caddy certificates and config state. Mount those paths if you need persistence across container replacements.
- Frontend runtime settings can be overridden through `frontend/.env` using `PUBLIC_BACKEND_BASE_URL`.
