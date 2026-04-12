# Build Action 020: Domain Persistence And Book Workspace Management

## Goal

Implement the core backend data model and workspace provisioning logic so the system can represent conversations, books, jobs, and revisions in a durable way.

## Sequencing Note

When an agent implements this action, it should assume all earlier numbered build actions have already been completed and may be relied on as existing project context.

## Scope

This action should implement:

- domain entities from `domain-model.md`
- persistence schema and repository layer
- one-conversation-to-one-book invariants
- workspace provisioning for explicit book initialization
- canonical on-disk book structure based on `book-structure.md`
- Rust-native setup command handling for book creation

## Required Decisions

- choose the persistence technology for MVP
- define how conversation identity is normalized across messenger providers
- define the initial book workspace bootstrap template

## Acceptance Criteria

- A conversation can be created or resolved from normalized provider data.
- An explicit setup command provisions exactly one book workspace for a conversation.
- A second setup attempt for the same conversation does not create a duplicate book.
- A newly provisioned workspace contains `book.yaml`, `style.yaml`, and the expected content and asset directories.
- Book, session, job, revision, and repository binding records can be stored and queried through the backend.
- Book workspace paths remain isolated per conversation.

## Verification

### API Tests

- Add repository-layer tests for create, read, and uniqueness rules on conversations and books.
- Add workspace tests that assert the generated directory tree and starter files match the spec.
- Add command tests showing setup commands create books and non-setup commands do not implicitly create books.

### End-To-End API Tests

- Execute a backend integration test that sends a setup command through the normalized command path and verifies:
  - the database state is persisted correctly
  - the workspace exists on disk
  - the response is a user-facing success result

### Frontend Verification

- Not required beyond confirming the reader shell still launches against the test backend after persistence is introduced.
