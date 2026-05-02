# Build Plan Index

## Purpose

This folder breaks the current high-level product specifications into an ordered implementation plan.

Each numbered file is a separate build action specification with:

- implementation goal
- scope and major decisions
- acceptance criteria
- verification approach

## Sequence

1. `010-foundation-and-workspace.md`
2. `020-domain-persistence-and-book-workspace.md`
3. `030-reader-api-and-render-pipeline.md`
4. `040-frontend-reader-shell.md`
5. `041-web-authentication.md`
6. `042-web-book-provisioning-and-conversation-registry.md`
7. `043-web-conversation-session-transcript-api.md`
8. `044-web-messenger-shell-and-conversation-list.md`
9. `045-reader-selection-mention-flow.md`
10. `050-messenger-adapters-and-command-routing.md`
11. `060-agent-execution-and-job-lifecycle.md`
12. `070-end-to-end-authoring-flow.md`
13. `071-app-module-refactor.md`
14. `075-deployment-packaging-and-runtime.md`
15. `080-observability-hardening-and-release-readiness.md`

## Established Conventions

Build action `010-foundation-and-workspace.md` fixes the baseline repository conventions that later actions should inherit rather than re-decide:

- local book workspaces live under `books-data/`
- backend local development runs as a host `cargo run` process
- frontend local development runs as a host `npm run dev` process
- `make up` delegates to `build/dev-up.sh`
- `make build` delegates to the combined Docker build in `build/Dockerfile`
- backend test mode uses distinct filesystem roots under `target/test/`
- frontend-to-backend local integration uses `PUBLIC_BACKEND_BASE_URL`

## Verification Strategy

The plan assumes three verification layers:

- Rust API and module tests for contract-level backend behavior
- end-to-end backend tests that exercise real request flows and workspace effects
- browser verification through a Puppeteer MCP flow that opens Chromium, loads the reader UI, clicks through the visible states, and captures screenshots where useful

## Delivery Rule

An action should be considered complete only when its own acceptance criteria and verification steps pass without depending on manual interpretation.
