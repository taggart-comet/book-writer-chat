# Build Action 044: Web Messenger Shell And Conversation List

## Goal

Build the authenticated frontend shell for the web messenger, including book selection, left-menu navigation, conversation listing, and transcript display.

## Sequencing Note

When an agent implements this action, it should assume build actions `040`, `041`, `042`, and `043` have already been completed.

This action should be completed before the reader can target a latest active conversation from a text selection.

## Scope

This action should implement:

- authenticated frontend shell with left-side navigation
- books view and selected-book context
- messenger button in the left menu
- conversation list for the selected book
- create-conversation action
- transcript view for the selected conversation
- loading, empty, and error states for the messenger area
- polling-based transcript refresh for MVP if live push is not yet available

This step should not yet implement reader-to-messenger selection handoff.

## Required Decisions

- choose the route structure for books and messenger views
- choose whether selected book state is URL-driven, store-driven, or hybrid
- choose the MVP polling interval for transcript refresh
- choose how the latest active conversation is labeled in the UI

## Acceptance Criteria

- The authenticated shell includes a left menu with a messenger entry.
- After a book is created and selected, opening messenger shows the conversation list for that book.
- The user can create a new conversation from the messenger view.
- Selecting a conversation loads and displays its normalized transcript.
- Empty, loading, and backend-error states are rendered clearly.
- The UI can identify the latest active conversation using `last_active_at` returned by the backend.
- Creating a conversation updates the visible list without a full page reload.

## Verification

### API Tests

- Keep the contract tests from `041` through `043` green for regression coverage.

### End-To-End API Tests

- Drive the authenticated frontend against the real backend: create a book, open messenger, create a conversation, and confirm the conversation list and transcript are loaded from the public APIs.

### Frontend Verification

- Use Chromium to verify:
  - login succeeds
  - a book can be selected
  - the messenger menu entry opens the conversation area
  - a new conversation appears in the list
  - the latest active conversation is visually identifiable
  - screenshots are captured for empty, populated, and error states
