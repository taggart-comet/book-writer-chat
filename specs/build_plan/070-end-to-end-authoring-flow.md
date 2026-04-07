# Build Action 070: End-To-End Authoring Flow

## Goal

Prove that the integrated system works from messenger command through backend orchestration to browser rendering for the main user journey.

## Sequencing Note

When an agent implements this action, it should assume all earlier numbered build actions have already been completed and may be relied on as existing project context.

## Scope

This action should connect the implemented pieces into a single validated flow:

- explicit conversation setup
- authoring request submission
- job execution
- revision creation
- link generation
- browser rendering of the updated draft

## Required Decisions

- choose the canonical end-to-end test harness for backend, frontend, and browser automation
- choose the sample conversation and sample manuscript scenarios used for regression coverage

## Acceptance Criteria

- A new conversation can be initialized explicitly through the command flow.
- A later bot-directed writing instruction updates the conversation-owned workspace.
- The messenger reply includes a working frontend link for the current draft.
- Opening that link shows the updated manuscript content.
- The system still ignores non-bot conversation chatter.
- The same flow works without leaking state between two different conversation fixtures.

## Verification

### API Tests

- Keep focused unit and contract tests from earlier steps green as regression coverage.
- Preserve the existing backend end-to-end regression harness under `src/app/test_support.rs` unless there is a deliberate decision to replace it with a clearer equivalent.

### End-To-End API Tests

- Add a full-system integration test covering:
  - setup command
  - writing command
  - resulting revision lookup
  - reader API fetch for the updated draft
- Add a second integration test with two conversations to prove workspace isolation.

### Frontend Verification

- Run Puppeteer MCP against the real local stack and verify:
  - the returned link opens in Chromium
  - the book title and newly written content are visible
  - navigation or content continuation works if applicable
  - screenshots are captured for the main rendered state
