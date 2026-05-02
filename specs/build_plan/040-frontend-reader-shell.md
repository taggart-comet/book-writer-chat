# Build Action 040: Frontend Reader Experience

## Goal

Build the Svelte reading interface that presents the current book draft, revision freshness, and backend job state in a polished browser experience.

## Sequencing Note

When an agent implements this action, it should assume all earlier numbered build actions have already been completed and may be relied on as existing project context.

## Scope

This action should implement:

- book reading route and layout
- integration with reader API endpoints
- loading, empty, updating, and error states
- chapter or content continuation navigation
- visible revision freshness and job status indicators
- selection actions for copying rendered text and opening the `Упомянуть эти строки` menu for messenger feedback
- a design direction that feels intentional and book-like rather than generic app chrome

## Required Decisions

- choose whether initial rendering is server-side, client-side, or hybrid
- choose how chapter navigation or infinite continuation behaves in MVP
- choose how to present active job progress without visual noise

## Acceptance Criteria

- Opening a valid reader link displays the latest book draft in a readable layout.
- The reader handles empty books and render errors gracefully.
- The frontend can load large book content incrementally using the backend contract.
- Revision freshness and recent job state are visible without exposing backend internals.
- The route structure supports direct linking to a specific book view.
- Selecting rendered book text exposes copy text and `Упомянуть эти строки` actions.
- The `Упомянуть эти строки` action offers `in a new conversation` and `in the latest active conversation` options.
- The implementation uses only approved dependency versions from the reference project.

## Verification

### API Tests

- Add frontend-facing contract tests at the backend boundary if needed to lock response shapes used by the UI.

### End-To-End API Tests

- Run a full backend-plus-frontend integration where seeded book data is retrieved by the frontend route and displayed successfully.

### Frontend Verification

- Use Puppeteer MCP to open Chromium and verify:
  - the reader page loads successfully from a real link
  - chapter or content continuation works
  - selecting rendered text exposes copy text and `Упомянуть эти строки` actions
  - empty and error states are rendered correctly
  - screenshots can be captured for the main reading view and at least one alternate state
