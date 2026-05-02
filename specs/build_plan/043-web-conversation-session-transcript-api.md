# Build Action 043: Web Conversation Session Transcript API

## Goal

Create the backend conversation APIs that launch Codex-backed conversations, persist conversation metadata, and expose normalized transcript messages from session logs.

## Sequencing Note

When an agent implements this action, it should assume build actions `041` and `042` have already been completed.

This action should be completed before the frontend messenger conversation view is wired.

## Scope

This action should implement:

- authenticated `GET /api/books/:book_id/conversations`
- authenticated `POST /api/books/:book_id/conversations`
- authenticated `GET /api/books/:book_id/conversations/:conversation_id/messages`
- creation and persistence of conversation records in `conversations.json`
- persistence of `created_at`, `updated_at`, `last_active_at`, `status`, and `session_log_path`
- Codex session launch scoped to the selected book workspace
- backend parsing of Codex JSONL session logs into normalized transcript items

This step should not yet implement a polished messenger UI or the reader mention action.

## Required Decisions

- choose how a new Codex session is launched and how its session log path is captured
- choose the canonical conversation status values for the web messenger flow
- choose how to derive stable message identifiers from JSONL events
- choose how partially written session logs are handled during reads

## Acceptance Criteria

- Creating a conversation requires JWT authentication.
- Creating a conversation launches a Codex session bound to the selected book workspace.
- The resulting conversation record is stored in that book’s `conversations.json`.
- Each conversation record includes `last_active_at`.
- `GET /api/books/:book_id/conversations` returns conversation metadata including `last_active_at`.
- `GET /api/books/:book_id/conversations/:conversation_id/messages` returns the current conversation `status` plus normalized transcript items derived from the stored session log path.
- The backend parses JSONL directly and does not depend on a shell `cat | jq` pipeline.
- Session log paths are validated so one book cannot read arbitrary files or another book’s conversation transcript.

## Verification

### API Tests

- Add tests for conversation creation, conversation listing, transcript reads, missing session log handling, malformed JSONL handling, and path-validation failures.
- Add tests proving `last_active_at` is updated when a conversation receives a new user-visible prompt or Codex message.
- Add tests proving unsupported JSONL event types are ignored.

### End-To-End API Tests

- Create a book, create a conversation, seed or generate a session log, then fetch the normalized message list through the public API and assert stable ordering and content.

### Frontend Verification

- Use Chromium to hit the authenticated backend through a temporary inspection page or dev UI, create a conversation, load its messages, and capture screenshots of both empty and populated transcript states.
