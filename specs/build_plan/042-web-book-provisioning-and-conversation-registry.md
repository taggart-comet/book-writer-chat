# Build Action 042: Web Book Provisioning And Conversation Registry

## Goal

Allow an authenticated user to create a book workspace from the web UI and establish the per-book `conversations.json` registry used by the messenger flow.

## Sequencing Note

When an agent implements this action, it should assume build actions `010`, `020`, and `041` have already been completed.

This action should be completed before new web conversations can be created.

## Scope

This action should implement:

- authenticated `GET /api/books` and `POST /api/books`
- explicit book creation under `books-data/`
- safe slug generation from a user-facing title
- workspace bootstrap using the approved book structure
- creation of `conversations.json` in each new book workspace
- backend-owned conversation registry reads and writes

This step should not yet launch Codex sessions or parse transcript logs.

## Required Decisions

- choose the exact bootstrap source for starter files
- choose the canonical JSON structure for `conversations.json`
- choose whether book listing is filesystem-derived, registry-derived, or persisted separately

## Acceptance Criteria

- Creating a book requires JWT authentication.
- Creating a book creates a dedicated folder under `books-data/`.
- The created folder contains the expected starter workspace structure.
- The created folder contains `conversations.json`.
- `conversations.json` is initialized as valid structured data rather than an ad hoc text file.
- The backend exposes a book list that includes newly created books without requiring manual filesystem intervention.
- Book creation remains explicit and is not triggered implicitly by opening messenger views.

## Verification

### API Tests

- Add tests for successful book creation, duplicate-slug handling, invalid title handling, and unauthenticated access rejection.
- Add tests proving the created workspace stays under `books-data/` and cannot escape through crafted input.
- Add tests proving `conversations.json` is created with the expected empty initial structure.

### End-To-End API Tests

- Create a book through the public API, then assert the workspace, starter files, and conversation registry exist on disk and can be listed through `GET /api/books`.

### Frontend Verification

- Open the authenticated app shell in Chromium, create a book from the UI, confirm the new book appears in the list, and capture a screenshot of the success state.
