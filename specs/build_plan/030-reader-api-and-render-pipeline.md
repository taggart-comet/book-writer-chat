# Build Action 030: Reader API And Render Pipeline

## Goal

Expose backend reader endpoints and a deterministic render pipeline that converts a book workspace into frontend-consumable content.

## Sequencing Note

When an agent implements this action, it should assume all earlier numbered build actions have already been completed and may be relied on as existing project context.

## Scope

This action should implement:

- book summary endpoint
- book content endpoint with chunking or chapter-based loading
- revision endpoint
- job status endpoint
- render assembly from Markdown and YAML workspace sources
- render snapshot generation or caching strategy for deterministic delivery

## Required Decisions

- choose whether the reader API returns structured JSON, rendered HTML fragments, or both
- choose the initial chunking mechanism for large books
- define how render failures are surfaced to the frontend
- implement the secure signed reader-link model defined in `backend-reader-api.md`

## Acceptance Criteria

- The backend can read a valid workspace and assemble a deterministic render output for the latest revision.
- The reader API exposes book metadata separately from content.
- The content endpoint supports incremental retrieval through chapter identifiers or cursors.
- The reader API does not expose raw internal file paths as part of its public contract.
- Revision and job endpoints expose enough state for the frontend to display freshness and in-progress work.
- Render failures produce explicit error states instead of silent partial output.
- Reader links use signed expiring tokens rather than uncontrolled public identifiers.

## Verification

### API Tests

- Add tests for each reader endpoint covering success, missing book, stale revision, and render failure cases.
- Add render pipeline tests proving identical workspace input produces identical render output.
- Add content pagination or chapter-loading tests proving the continuation mechanism is stable.

### End-To-End API Tests

- Seed a realistic sample workspace, call the public reader endpoints in order, and assert the returned content matches the expected logical structure and metadata.

### Frontend Verification

- Point the frontend to a seeded backend and use Puppeteer MCP to open the reader URL, confirm content appears, and capture screenshots of:
  - successful render state
  - loading state
  - render error state
