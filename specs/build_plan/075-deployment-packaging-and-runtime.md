# Build Action 075: Deployment Packaging And Runtime

## Goal

Implement the deployment model defined in `deployment.md` so the application can run as one deployable Docker image containing the Rust backend, built Svelte frontend, and Caddy.

## Sequencing Note

When an agent implements this action, it should assume all earlier numbered build actions have already been completed and may be relied on as existing project context.

## Scope

This action should implement:

- a deployment-oriented `build/` directory
- combined multi-stage Docker build for backend, frontend, and runtime assembly
- a `Caddyfile` that routes `/api/*` and operational endpoints to the Rust backend
- frontend asset serving for browser routes
- a runtime entrypoint or similarly explicit process startup mechanism for Caddy and the backend
- environment-driven runtime configuration
- persistent-data path expectations for the books root and Caddy state
- a working root `Makefile` where `make build` produces the combined image

## Required Decisions

- choose whether frontend assets are served directly by Caddy or by the backend behind Caddy
- choose the runtime startup mechanism for multiple processes inside the container
- choose the internal backend bind port and container-facing public port behavior
- define the minimum production environment variables and mounted directories

## Acceptance Criteria

- The repository contains `build/Dockerfile`, `build/Caddyfile`, and any required startup script or process-launch file.
- `make build` produces a single Docker image containing the backend binary, built frontend assets, and Caddy runtime configuration.
- Starting the built image runs both Caddy and the Rust backend successfully.
- Browser routes resolve to the frontend application and API routes resolve to the backend through Caddy.
- Health and readiness endpoints remain reachable through the deployed routing layer.
- The runtime configuration is environment-driven rather than hardcoded.
- The deployment model documents or enforces how persistent storage is mounted for book workspaces and Caddy state.

## Verification

### API Tests

- Add configuration tests for production-oriented environment parsing used by the container runtime.
- Add tests for any backend assumptions required by Caddy routing, such as health endpoints and internal bind behavior.

### End-To-End API Tests

- Build the combined Docker image in CI or a local deployment test flow.
- Start the container with test environment variables and assert:
  - the container becomes healthy
  - `/api/*` requests reach the backend successfully
  - browser application routes return the frontend content
  - book data remains accessible through the routed reader endpoints

### Frontend Verification

- Use Puppeteer MCP against the running combined container, not the separate dev servers, and verify:
  - the reader route loads through Caddy over the deployed routing shape
  - API-backed content still renders correctly
  - screenshots can be captured from the deployment-like runtime
