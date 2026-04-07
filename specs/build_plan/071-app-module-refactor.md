# Build Action 071: Backend Module Refactor With Explicit `src/` Structure

## Summary

Refactor the backend from the current flat `src/` layout into a small number of domain-oriented module trees. The key split is:

- `messaging/`: inbound provider normalization, command parsing, webhook entrypoints, conversation command flow
- `reader/`: reader APIs, signed reader-link HTML delivery, render snapshot loading, cursor handling
- `authoring/`: prompt construction, executor integration, job lifecycle orchestration, revision/render persistence
- `app/`: top-level composition only
- `core/`: shared types/utilities that are truly cross-cutting
- `storage/`: persistence and workspace filesystem access

The refactor is structural only. It must preserve all current routes, API payloads, tests, and runtime behavior.

## New `src/` Structure

Proposed target tree:

```text
src/
  main.rs
  lib.rs

  app/
    mod.rs
    state.rs
    router.rs
    metrics.rs
    errors.rs
    test_support.rs

  messaging/
    mod.rs
    providers/
      mod.rs
      telegram.rs
      max.rs
    commands.rs
    handlers.rs

  authoring/
    mod.rs
    prompt.rs
    executor.rs
    flow.rs

  reader/
    mod.rs
    links.rs
    handlers.rs
    html.rs
    content.rs

  storage/
    mod.rs
    repository.rs
    workspace.rs
    render_store.rs

  core/
    mod.rs
    config.rs
    models.rs
```

## Exact File Moves

### `app/`
Purpose: composition only.

- `app/mod.rs`
  - re-export `build_router`
  - declare child modules
- `app/router.rs`
  - current `build_router`
  - route registration only
  - `health`, `ready`, `metrics`
  - static frontend fallback wiring
- `app/state.rs`
  - `AppState`
  - conversation lock map helpers
- `app/metrics.rs`
  - `Metrics` struct and counter methods
- `app/errors.rs`
  - `api_error`
  - any shared HTTP error helpers used by reader or messaging handlers
- `app/test_support.rs`
  - `test_app`
  - request helpers like `post_telegram`, `post_max`, `get`, `read_json`
  - fixture loader helpers
  - test payload builders

### `messaging/`
Purpose: everything about inbound bot-directed messages.

- `messaging/providers/telegram.rs`
  - Telegram payload structs
  - Telegram normalization logic from current `messenger.rs`
  - `telegram_mentions_bot`
- `messaging/providers/max.rs`
  - MAX payload structs
  - MAX normalization logic from current `messenger.rs`
- `messaging/providers/mod.rs`
  - provider trait if retained
  - `normalize_telegram`
  - `normalize_max`
- `messaging/commands.rs`
  - current `ParsedCommand`
  - `parse_command`
- `messaging/handlers.rs`
  - `telegram_webhook`
  - `max_webhook`
  - `message_flow`
  - `init_flow`
  - `status_flow`
  - messaging-facing response construction
  - call into `authoring::flow` for authoring commands
- `messaging/mod.rs`
  - wire exports needed by router

### `authoring/`
Purpose: authoring job execution lifecycle.

- `authoring/prompt.rs`
  - move current `prompting.rs` contents unchanged in behavior
- `authoring/executor.rs`
  - move current `agent.rs` contents unchanged in behavior
- `authoring/flow.rs`
  - `authoring_flow`
  - `finalize_authoring`
  - `seed_initial_render`
  - `persist_render_snapshot`
  - `revision_summary`
  - any job-state transition helpers
- `authoring/mod.rs`
  - wire exports for app/messaging usage

### `reader/`
Purpose: signed links, reader APIs, reader HTML, and chapter/cursor logic.

- `reader/links.rs`
  - move current `reader_links.rs`
- `reader/handlers.rs`
  - `reader_summary`
  - `reader_content`
  - `reader_revision`
  - `reader_job`
  - `resolve_book_for_token`
  - `load_latest_rendered_book`
- `reader/html.rs`
  - `reader_shell`
  - `render_reader_shell_html`
  - `escape_html`
- `reader/content.rs`
  - `ContentQuery`
  - `ChapterCursor`
  - `requested_chapter_index`
  - `encode_cursor`
  - `decode_cursor`
- `reader/mod.rs`
  - wire exports needed by router

### `storage/`
Purpose: data persistence and filesystem-backed book/render access.

- `storage/repository.rs`
  - move current `persistence.rs`
- `storage/workspace.rs`
  - move current `workspace.rs`
- `storage/render_store.rs`
  - move current `render.rs`
- `storage/mod.rs`
  - wire exports

### `core/`
Purpose: stable shared definitions.

- `core/config.rs`
  - move current `config.rs`
- `core/models.rs`
  - move current `models.rs`
- `core/mod.rs`
  - wire exports

## `lib.rs` Changes

`lib.rs` should stop exposing a flat module list. Replace it with:

```rust
pub mod app;
pub mod authoring;
pub mod core;
pub mod messaging;
pub mod reader;
pub mod storage;
```

No internal module should import old flat paths after the refactor. All imports must use the new tree.

## What Leaves `app.rs`

Current `src/app.rs` should be reduced to either:

1. `src/app/mod.rs` with only module declarations and re-exports, or
2. a very small `src/app.rs` that only exposes `build_router`

Chosen direction: use `src/app/mod.rs`.

Everything currently in `app.rs` moves out as follows:

- Router building and health/readiness/metrics endpoints -> `app/router.rs`
- `AppState`, `Metrics`, lock storage -> `app/state.rs` and `app/metrics.rs`
- Telegram/MAX webhook handlers and message routing -> `messaging/handlers.rs`
- Init/status command behavior -> `messaging/handlers.rs`
- Authoring execution pipeline -> `authoring/flow.rs`
- Reader API handlers -> `reader/handlers.rs`
- Reader shell HTML route + renderer helpers -> `reader/html.rs`
- Cursor parsing/encoding and chapter selection helpers -> `reader/content.rs`
- Test module -> split so helpers go in `app/test_support.rs`; end-to-end tests can remain under `app/router.rs` test module or move to dedicated integration-style test modules under the new domain modules, but production code files must not carry the whole current helper pile

## Interfaces And Boundaries

Use these boundaries to avoid recreating coupling:

- `app/router.rs` depends on:
  - `app::state`
  - `messaging::handlers`
  - `reader::handlers`
  - `reader::html`
- `messaging::handlers` depends on:
  - `app::state`
  - `messaging::commands`
  - `messaging::providers`
  - `authoring::flow`
  - `reader::links`
  - `storage::repository`
  - `storage::workspace`
  - `core::models`
- `authoring::flow` depends on:
  - `app::state`
  - `authoring::executor`
  - `authoring::prompt`
  - `reader::links`
  - `storage::repository`
  - `storage::workspace`
  - `storage::render_store`
  - `core::models`
- `reader::handlers` and `reader::html` depend on:
  - `app::state`
  - `reader::links`
  - `reader::content`
  - `storage::repository`
  - `storage::render_store`
  - `core::models`

Rule: `reader/` must not depend on `messaging/`. `messaging/` must not depend on reader handler implementations. Shared HTTP helpers stay in `app/errors.rs` or move to `core` only if they are not HTTP-specific.

## Test Plan

- Keep all existing Rust tests green.
- Preserve the current end-to-end coverage:
  - setup command creates conversation workspace
  - non-bot chatter is ignored
  - authoring updates workspace and persists revision
  - reader APIs return latest content
  - signed reader link renders real draft content
  - two conversations stay isolated
- Move test helpers out of the main production composition file.
- Add one small smoke test for router assembly after extraction if needed to keep route wiring obvious.

## Assumptions And Defaults

- This refactor does not change route paths, request/response JSON, token format, persistence schema, workspace layout, or frontend behavior.
- `commands.rs`, `messenger.rs`, `agent.rs`, `prompting.rs`, `reader_links.rs`, `render.rs`, `persistence.rs`, `workspace.rs`, `config.rs`, and `models.rs` are removed as flat top-level modules after their contents are moved.
- The preferred organization is domain-first, not layer-first. That is why messenger code goes under `messaging/` and reader/render code goes under `reader/`, instead of keeping all handlers/helpers as same-level peers.
- The implementer should do the refactor in small compile-green steps, but the end state must match the module structure above rather than inventing a different tree.
