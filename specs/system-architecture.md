# System Architecture

> Status: This is a high-level specification and is read-only by default. It should be changed only with explicit approval from the engineer.

## High-Level Components

The system consists of four main parts:

1. Messenger integrations
2. Rust backend
3. Codex CLI execution layer
4. Frontend built with Svelte

## Responsibility Split

### 1. Messenger Integrations

Messenger integrations receive inbound user messages and deliver outbound replies.

At first, we should support:

- Telegram
- MAX (Russian state owned)

These integrations should be treated as adapters around a common internal message contract.

### 2. Rust Backend

The Rust backend has two major responsibilities:

- ingest commands from messenger platforms and orchestrate Codex CLI execution
- serve APIs and assets required by the frontend so conversation participants can view the current book draft

It should model books by conversation, not by individual user account.

### 3. Codex CLI Execution Layer

Codex CLI is the writing engine. It receives:

- the user instruction
- system prompt additions
- book/workspace context
- optional recent conversation context

It then updates book project files in a controlled workspace and returns execution status and output metadata.

### 4. Svelte Frontend

The frontend renders the user’s current book draft and related status information. It should optimize for readability, aesthetic presentation, and clarity of draft state.

The frontend technology and dependency policy are specified separately in `frontend.md`.

## Conceptual Runtime Flow

1. A messenger webhook or polling adapter receives a user command.
2. The backend authenticates and normalizes the incoming message.
3. The backend checks whether the message targets the bot.
4. Non-bot-directed conversation messages are ignored.
5. Bot-directed messages are mapped to a conversation, book, and active authoring session.
6. The backend either handles the message as a Rust-native control command or creates a writing job.
7. For writing jobs, the backend invokes Codex CLI against that conversation’s book workspace.
8. The agent updates the book project.
9. The backend captures resulting revision metadata and render state.
10. The backend sends a messenger reply containing a status summary and frontend URL.
11. The frontend fetches book data from the backend and renders the latest draft.

## Suggested Deployable Services

For MVP, the backend can be a single Rust application exposing:

- webhook or polling endpoints for messenger providers
- internal orchestration modules for agent execution
- public or authenticated HTTP endpoints for frontend data
- static asset or SSR support for the frontend deployment boundary

The Codex CLI execution may run:

- in-process as a child process launched by the Rust backend
- on the same host but isolated by per-book workspace directories

## Data Boundaries

### Backend-owned data

- conversations
- books
- authoring sessions
- writing jobs
- revisions
- render snapshots or render metadata
- repository linkage metadata

### Agent-owned mutable workspace

- book source files
- generated manuscript assets
- revision artifacts
- agent logs or transcripts if retained

Each book workspace should live under a local books root such as `books/` or `books-data/`, with one directory per conversation/book.

### Frontend-consumed data

- book metadata
- current rendered content
- revision timestamps
- job status
- shareable or authenticated access URLs

The frontend contract should be render-oriented and chunk-friendly. Internal manuscript files and workspace layout should remain backend concerns rather than public API details.

## Architectural Constraints

- Rust is the system-of-record backend.
- Svelte is the frontend framework.
- Messenger platforms must be pluggable, not hardcoded.
- The system should not assume a single book structure format yet.
- Every write action must be attributable to a user command and a backend job.
- The primary identity key is the messenger conversation, not the individual user.
- One conversation maps to one book workspace directory.
- Local book workspace directories must be ignored by Git so private manuscript data does not leak into the repository.

## Cross-Cutting Concerns

- workspace isolation per conversation/book
- job idempotency and retry handling
- observability and auditability
- rate limiting and abuse prevention
- secure prompt construction and command sanitization
- repository provisioning and linkage for per-book GitHub repositories

## Open Questions

- Should frontend rendering rely on pre-rendered HTML, structured JSON, or both?
- Should the backend generate signed viewer URLs, authenticated sessions, or simple opaque links?
- Will Codex execution be synchronous for MVP, or queued asynchronously?
- What exact Rust-native control commands should be supported at launch for setup and repository management?
