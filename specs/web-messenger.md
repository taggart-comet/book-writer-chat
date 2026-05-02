# Web Messenger Specification

> Status: This is a high-level specification and is read-only by default. It should be changed only with explicit approval from the engineer.

## Purpose

This specification defines a web-native messenger experience for `book-writer-chat` so the system can be used even when external messenger providers are unavailable or blocked.

The web messenger becomes an authenticated browser interface that:

- allows an operator to sign in
- creates and manages book workspaces under `books-data/`
- launches Codex sessions inside a selected book workspace
- lists conversations for a book
- renders conversation history from stored Codex session logs

## Product Direction

The web messenger is a first-party browser replacement for the external chat surface, not a separate authoring model.

The core interaction remains the same:

- a user selects a book workspace
- a user opens or creates a conversation for that book
- the backend launches or resumes a Codex session scoped to that workspace
- the frontend displays the resulting transcript and supports continued prompting

This feature does not replace the existing reader experience. It adds an authenticated authoring surface alongside it.

## Authentication

All web messenger API requests must require JWT-based authentication.

The first version uses one operator account only.

Rules:

- the allowed username is provided by environment configuration
- the allowed password is provided by environment configuration
- credentials must not be hardcoded in source code
- credentials are fixed for the running deployment and are not self-service
- successful login returns a signed JWT
- the frontend stores and reuses the JWT for subsequent API calls
- unauthenticated requests to protected endpoints return `401 Unauthorized`
- malformed, expired, or invalid tokens return `401 Unauthorized`

Required environment variables:

- `WEB_AUTH_USERNAME`
- `WEB_AUTH_PASSWORD`
- `JWT_SIGNING_SECRET`

The backend must expose a dedicated login endpoint that validates the configured credentials and issues the token.

## Book Provisioning

The web UI must support explicit book creation.

Creating a book means creating a dedicated workspace folder under the default local root:

- `books-data/`

Each created book must have its own isolated directory.

Book creation must:

- require an authenticated request
- accept at least a user-facing book title
- generate a stable safe folder slug
- create the target folder under `books-data/`
- copy the standard book workspace structure and starter files into that folder
- initialize the files required for later Codex execution and frontend display
- create an empty conversation registry file named `conversations.json` inside the book folder if it does not already exist

The provisioning source may be a checked-in template directory or a Rust-native bootstrap routine, but the created result must match the repository’s approved book structure.

Book creation must remain explicit. Opening the messenger UI must not implicitly create a book.

## Conversation Ownership Model

For the web messenger flow, one book may contain multiple conversations.

Each conversation belongs to exactly one book workspace.

Each conversation record must include at least:

- `conversation_id`
- `book_id` or stable book slug
- `title`
- `session_log_path`
- `created_at`
- `updated_at`
- `last_active_at`
- `status`

The conversation registry for a book is stored in:

- `books-data/<book-slug>/conversations.json`

That file is the backend-owned lookup table that maps book-local conversation metadata to Codex session log files.

## Conversation Registry Contract

`conversations.json` must be treated as backend-owned structured data.

Minimum requirements:

- it must live in the root of each book workspace
- it must be created during book provisioning
- it must contain an array or equivalent structured collection of conversation records
- each record must point to exactly one Codex session log file
- each record must persist a `last_active_at` timestamp used to identify the latest active conversation for reader-to-messenger mention actions
- paths must be validated so a conversation cannot read arbitrary files outside the intended session storage scope

The backend must treat `conversations.json` as the source of truth for which conversations are visible in the web UI for that book.

`last_active_at` must be updated whenever a conversation receives a new user-visible Codex message or a new user prompt is attached to that conversation.

## Codex Session Launching

Creating a new conversation from the web UI must create a new Codex session scoped to the selected book workspace.

The backend launch flow must:

- require authentication
- resolve the target book workspace under `books-data/`
- launch Codex with that workspace as the working directory
- capture the resulting session identifier and session log path
- persist the conversation record into `conversations.json`
- return the created conversation metadata to the frontend

The launch contract must preserve the existing backend rule that one workspace is isolated from another.

## Frontend Navigation

The authenticated application shell must include a left-side navigation area.

The web messenger UI must follow the project-wide mobile-first frontend direction.

This means:

- the messenger shell must remain fully usable on phone-sized screens
- book selection, conversation selection, and transcript reading must work without requiring desktop-width layouts
- navigation may adapt across breakpoints, but it must preserve access to the same core messenger actions on mobile and desktop

Minimum navigation entries:

- books
- messenger
- reader, if the existing reader remains exposed in the same shell

After a book is created and selected, the messenger button in the left menu must open the messenger area for that book.

## Messenger UI Behavior

The messenger area must display the list of conversations for the selected book.

Minimum required behavior:

- show existing conversations from `conversations.json`
- allow creating a new conversation
- allow selecting one conversation to open its transcript
- identify which conversation is the latest active conversation based on `last_active_at`
- show loading, empty, and error states clearly
- refresh the visible transcript after new backend messages are available

The messenger area and any reader-originated mention action must use `last_active_at` to determine the current latest active conversation for the selected book.

The first version does not require multi-user presence, typing indicators, or live websocket delivery. Polling is acceptable for MVP.

## Transcript Source Of Truth

Each conversation transcript is derived from the Codex session output log referenced by that conversation’s `session_log_path`.

The user-provided extraction rule is:

```sh
cat ~/.codex/sessions/your_session.jsonl | jq -c 'select(.type=="event message" or .type=="response item") | {role: .ro, text: (.message // .content)}'
```

For backend implementation, this should be treated as a logical extraction contract rather than a required shell pipeline.

The Rust backend should parse the JSONL session file directly and return only the frontend-safe message stream equivalent to:

- records whose type is `event message` or `response item`
- a normalized `role`
- normalized text content from `message` or `content`

If the session schema uses a field other than `.ro` for role, the backend must map the actual source field to the normalized `role` field and not propagate a mistaken raw key into the public API.

## Transcript API Contract

The backend must expose an authenticated endpoint that returns the current conversation status together with normalized conversation messages for one conversation.

The response should include:

- `status` for the current conversation, using the same backend status vocabulary as the conversation list
- `messages`, containing the normalized transcript items

Each returned message inside `messages` should include at least:

- `message_id` or stable derived key
- `role`
- `text`
- `timestamp` if available from the session log

The backend must:

- read the session log path from `conversations.json`
- validate the path before opening it
- ignore unsupported JSONL records
- handle missing or partially written log files gracefully
- return messages in stable chronological order

If a session log is missing, unreadable, or malformed, the endpoint must return a controlled backend error and the frontend must show a readable failure state.

## Suggested HTTP API Surface

Minimum authenticated endpoints:

- `POST /api/auth/login`
- `GET /api/books`
- `POST /api/books`
- `GET /api/books/:book_id/conversations`
- `POST /api/books/:book_id/conversations`
- `GET /api/books/:book_id/conversations/:conversation_id/messages`

When listing conversations, the backend response must include `last_active_at` so the frontend can label or target the latest active conversation without inspecting session logs directly.

Possible later endpoints:

- `POST /api/books/:book_id/conversations/:conversation_id/messages`
- `POST /api/books/:book_id/conversations/:conversation_id/resume`

The first version may remain read-only for transcript retrieval after session creation if message sending is specified separately.

## Security Requirements

- Every web messenger endpoint except login must require JWT authentication.
- JWT signing secrets must come from environment configuration.
- Book paths and session log paths must be validated against allowed roots.
- The backend must not trust any client-provided filesystem path.
- One book’s conversations must not expose another book’s session logs.
- Error responses must avoid leaking secrets or host-specific sensitive paths.

## UX Requirements

- Login must fail with a concise invalid-credentials message.
- Creating a book must give immediate visible confirmation.
- Opening messenger for a book must show either the conversation list or a clear empty state.
- Creating a conversation must add it to the list without requiring a full page reload.
- Opening a conversation must show the normalized Codex transcript in message order.
- Reader-originated `Упомянуть эти строки` actions must let the user choose between a new conversation and the latest active conversation for that book.

## Non-Goals For This Version

- multi-user accounts and permissions
- registration, password reset, or user management
- replacing the existing external messenger adapter architecture
- exposing raw Codex JSONL records directly to the frontend
- arbitrary filesystem browsing for session logs

## Open Questions

- Should web messenger messages be sent into a persistent interactive Codex session, or should each user message launch a fresh bounded Codex run linked to the same conversation?
- Should transcript refresh be short-interval polling or server-sent events in MVP?
- What exact starter files must be copied into a newly created book workspace beyond the baseline structure and empty `conversations.json`?
