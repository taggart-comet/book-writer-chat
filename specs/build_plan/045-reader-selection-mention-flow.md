# Build Action 045: Reader Selection Mention Flow

## Goal

Connect the reader text-selection tools to the web messenger so selected book text can be mentioned into either a new conversation or the latest active conversation.

## Sequencing Note

When an agent implements this action, it should assume build actions `030`, `040`, `043`, and `044` have already been completed.

This action depends on conversation metadata and latest-active resolution already being available from the messenger APIs.

## Scope

This action should implement:

- replacement of the old reader copy-reference action with `Упомянуть эти строки`
- an action menu with exactly two options:
  - `in a new conversation`
  - `in the latest active conversation`
- generation of the structured manuscript reference payload from reader source metadata
- frontend resolution of the latest active conversation using `last_active_at`
- handoff of the generated mention payload into the selected conversation target

This step may stop at a verified handoff into the web messenger flow even if long-running interactive message sending evolves further later.

## Required Decisions

- choose the exact mention payload shape sent from reader context into the messenger flow
- choose whether the handoff opens messenger immediately or sends in place and then navigates
- choose how the UI behaves when no latest active conversation exists yet

## Acceptance Criteria

- Selecting rendered text exposes `Copy text` and `Упомянуть эти строки` actions.
- `Упомянуть эти строки` opens a menu with exactly the two specified options.
- Choosing `in a new conversation` creates a conversation target and attaches the generated mention payload to it.
- Choosing `in the latest active conversation` uses `last_active_at` to target the correct conversation for the current book.
- If no prior conversation exists, the latest-active option is either disabled with a clear reason or falls back according to an explicit MVP decision.
- The mention payload includes the source file path, line and character span, and quoted selected text.
- The implementation does not expose raw manuscript-file browsing beyond the explicit source-reference metadata already allowed by the reader contract.

## Verification

### API Tests

- Add contract tests if new backend endpoints or payload shapes are introduced for mention handoff.
- Keep latest-active conversation ordering and selection rules under automated test coverage.

### End-To-End API Tests

- Seed a rendered book and at least two conversations with different `last_active_at` values, trigger the reader mention flow, and assert the expected conversation target receives the structured selection payload.

### Frontend Verification

- Use Chromium to verify:
  - selecting rendered text shows the two actions
  - `Упомянуть эти строки` opens the expected menu
  - the new-conversation path works
  - the latest-active-conversation path targets the correct conversation
  - screenshots are captured for both menu and post-selection states
