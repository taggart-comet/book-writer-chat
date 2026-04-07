# Build Action 010: Foundation And Workspace Setup

## Goal

Establish the repository, runtime boundaries, local development layout, and implementation conventions needed for all later work.

## Sequencing Note

When an agent implements this action, it should treat this file as the first sequential build step.

There are no earlier build actions to assume as completed.

## Scope

This action should define and implement:

- Rust backend project entrypoint and module skeleton
- SvelteKit frontend project skeleton
- shared repository conventions for local development
- root `Makefile` with at least `make up` and `make build` targets stubbed or wired to the chosen local workflow
- Git ignore rules for conversation-owned book workspaces
- configuration loading for backend and frontend runtime settings
- initial deployment-oriented repository layout such as `build/`
- a documented local books root such as `books/` or `books-data/`

This step should not yet implement messenger behavior, Codex execution, or full reader rendering logic.

## Required Decisions

- choose the concrete books root directory name
- choose how backend and frontend are started in local development
- choose the initial `Makefile` strategy for local startup and image build delegation
- choose the minimum environment configuration structure for local and test execution

## Implemented Decisions

The current repository implementation resolves the required decisions as follows:

- the concrete local books root is `books-data/`
- local development starts the backend with `cargo run`
- local development starts the frontend with `npm run dev`
- `make up` delegates to `build/dev-up.sh`, which launches both host processes together
- `make build` delegates to the combined Docker image build using `build/Dockerfile`
- backend runtime configuration is environment-driven through `APP_ENV`, `APP_PORT`, `APP_DATA_DIR`, `APP_BOOKS_ROOT`, `FRONTEND_DIST_DIR`, and `FRONTEND_BASE_URL`
- frontend runtime configuration uses `PUBLIC_BACKEND_BASE_URL`
- backend `APP_ENV=test` defaults resolve into `target/test/data` and `target/test/books-data` without source edits

## Current Repository Note

The repository already contains some functionality that belongs to later build actions, including messenger routing and agent execution scaffolding.

For this action, those later capabilities should be treated as pre-existing implementation detail, not as a reason to reopen the foundation decisions above.

## Acceptance Criteria

- The repository contains a compilable Rust backend skeleton with named modules aligned with the architecture specs.
- The repository contains a runnable SvelteKit frontend skeleton with pinned dependency versions copied from `specs/frontend-approved-packages.md`.
- The local books root is ignored by Git.
- The repository contains a root `Makefile` with working `make up` and `make build` entrypoints, even if `make build` is initially a thin wrapper around the deployment build path.
- Backend configuration supports a distinct test environment without manual source edits.
- Frontend configuration supports a backend base URL or equivalent integration setting.
- A new developer can start both apps locally using documented commands.

## Verification

### API Tests

- Add backend tests proving configuration loading works for development and test modes.
- Add backend tests proving the configured books root resolves inside the intended local directory boundary.

### End-To-End API Tests

- Run a backend startup test that boots the application with test configuration and confirms the health or readiness endpoint responds successfully.

### Frontend Verification

- Launch the frontend and verify the root route renders a stable placeholder reader shell.
- Use Puppeteer MCP to open Chromium, load the root page, wait for the app shell, and capture a screenshot showing the initial reader layout.

## Documentation Follow-Through

The repository documentation for this action should stay aligned with the implemented decisions above:

- `README.md` should describe `make up`, `make build`, and the local runtime ports
- `.gitignore` should continue to exclude `books-data/` and local frontend env files
- future specs should refer to `books-data/` as the default local workspace root unless an explicit spec change is made
