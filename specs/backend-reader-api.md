# Backend Spec: Reader API And Frontend Delivery

> Status: This is a high-level specification and is read-only by default. It should be changed only with explicit approval from the engineer.

## Purpose

This part of the Rust backend serves the data needed by the Svelte frontend so conversation participants can inspect the current book draft in a browser after each change.

## Core Responsibilities

- expose frontend-consumable endpoints for book retrieval and status
- provide the latest rendered representation of a book draft
- expose revision and job metadata needed for UI feedback
- generate or validate links sent back through messenger

## Product Requirement

After a bot-directed messenger command is processed, the conversation should receive a link that opens a web view of the current draft in a polished reading experience.

## Reader Experience

The frontend should be able to display:

- book title and metadata
- current rendered book content
- revision freshness
- current job state if processing is ongoing
- any recoverable rendering or synchronization issues

## API Shape

The MVP routes are:

- `GET /api/reader/summary?token={token}`
- `GET /api/reader/content?token={token}`
- `GET /api/reader/content?token={token}&chapter_id={chapter_id}&revision_id={revision_id}`
- `GET /api/reader/content?token={token}&cursor={cursor}&revision_id={revision_id}`
- `GET /api/reader/revision?token={token}`
- `GET /api/reader/job?token={token}`

The API is JSON-first and returns rendered HTML fragments inside structured JSON payloads.

### Book Summary Endpoint

Returns lightweight data for the selected book.

Example fields:

- `book_id`
- `title`
- `subtitle`
- `status`
- `last_revision_id`
- `last_updated_at`
- `render_status`
- `chapter_count`

### Book Content Endpoint

Returns the current renderable book content.

The reader API must be render-oriented rather than source-oriented.

The frontend should not consume raw manuscript files as its primary contract. Instead, the backend should assemble the manuscript, resolve style and media metadata, and return display-ready content segments.

Because books may be large and text-heavy, content retrieval must support incremental loading.

Behavioral rules:

- the backend determines how much content to return per request
- the frontend may request a starting position, but not the response size
- the API exposes stable logical handles through chapter ids and revision-bound cursors
- internal Markdown filenames and storage layout must not be part of the public frontend contract
- if line-based offsets are used internally, they should remain a backend concern or be exposed only through an abstract cursor-like handle

MVP response strategy:

- structured JSON envelopes for frontend state
- pre-rendered chapter HTML fragments for book body content

A chunked response should include enough metadata for the frontend to continue loading, such as:

- `revision_id`
- `content_hash`
- chapter identifier or index
- returned HTML fragment
- `next_cursor`
- `has_more`

The cursor is opaque to the frontend and is bound to a specific revision. If the frontend sends a stale `revision_id` or a cursor from an older revision, the backend returns an explicit stale-revision error instead of silently mixing content from different renders.

### Revision Endpoint

Returns historical or latest revision metadata.

Example fields:

- `revision_id`
- `created_at`
- `source_job_id`
- `summary`
- `render_status`
- `content_hash`
- `render_error`

### Job Status Endpoint

Allows the frontend to poll or subscribe for recent write activity.

Example fields:

- `job_id`
- `status`
- `started_at`
- `finished_at`
- `user_facing_message`

## Linking Model

Messenger replies should include a stable frontend URL that identifies the conversation-owned book or current view.

For MVP, the reader link model is fixed as:

- signed single-book access links

The MVP must not rely on uncontrolled public book identifiers in URLs.

Required link behavior:

- the backend issues a signed token bound to one book
- the token includes an expiration time
- the frontend route carries the token, not a raw internal workspace path
- the backend validates the token before serving reader data
- invalid or expired tokens return an explicit structured access error response

Authentication may be added later, but MVP link delivery must already be secure enough for messenger-shared access.

## Rendering Pipeline

The backend and frontend need a clear contract for how manuscript source becomes rendered UI.

For MVP, the system should standardize on this approach:

1. The canonical manuscript source remains in the workspace.
2. The backend assembles and validates the manuscript.
3. The backend writes a deterministic render snapshot for each successfully rendered revision.
4. The backend exposes render-ready content through the reader API from the latest render snapshot.

This separates authoring storage from frontend presentation and allows the internal manuscript format to evolve without breaking the frontend contract.

## Svelte Frontend Requirements

The frontend should:

- fetch current book state from backend APIs
- render a readable, visually polished draft view
- handle loading, empty, error, and updating states cleanly
- support deep linking to a specific book

Later enhancements may include:

- revision comparison
- comments or annotations
- chapter navigation
- export flows

## Non-Functional Requirements

- low latency for loading the latest draft
- deterministic rendering of the same revision
- safe handling of stale or expired links
- clear degradation when a render is unavailable

## Error Model

Reader endpoint failures should be returned as structured JSON with:

- `code`
- `message`

Important MVP error codes include:

- `access_denied`
- `revision_not_found`
- `job_not_found`
- `chapter_not_found`
- `stale_revision`
- `render_failed`
- `render_snapshot_missing`
- `render_snapshot_invalid`

## Open Questions

- Should the frontend render server-side, client-side, or hybrid through SvelteKit?
- Should the book view auto-refresh while a job is running?
