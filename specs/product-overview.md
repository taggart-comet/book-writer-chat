# Product Overview

> Status: This is a high-level specification and is read-only by default. It should be changed only with explicit approval from the engineer.

## Purpose

`book-writer-chat` is a full-stack application that allows one or more non-technical participants to write a book through an in-app chat conversation.

The system receives user instructions from its own web messenger, enriches them with internal prompting, forwards them to a Codex CLI agent, and then presents the updated book in a web interface.

## Primary Context

The primary usage context is an in-app conversation in which one or more non-technical participants:

- prefers messaging over direct use of developer tools
- wants immediate visual feedback in a browser
- does not want to manage files, prompts, or source control manually
- may collaborate in the same chat on the same book

## Core Product Promise

The user should be able to:

1. Initialize book creation in the app with an explicit control command.
2. Send a natural-language instruction in the app messenger.
3. Submit that instruction into the book-writing flow.
4. Wait while the agent updates the book project.
5. Receive a link to a web view of the current book draft.
6. Open that link and see a beautifully rendered version of the current book.

## Example User Journey

1. A participant in the app sends the explicit control command that initializes a book for that conversation.
2. The backend creates or provisions the conversation-owned book workspace.
3. A participant then sends an instruction such as: `write an introductory chapter about habit formation for busy parents.`
4. The backend receives the message from the app messenger.
5. The backend resolves the conversation and its associated book workspace.
6. The backend adds system prompt context, project context, and recent conversation context.
7. The backend invokes a Codex CLI agent in the context of the conversation’s book workspace.
8. Messages in the same conversation that are not submitted as authoring commands are ignored.
9. The agent updates the book source files and metadata.
10. The backend records the resulting revision and generates a frontend URL.
11. The app surfaces job status and a link to the rendered book.
12. The participants open the web UI and see the updated draft.

If a participant sends a writing request before the book is initialized, the app should not create the book implicitly. It should instead show a concise setup instruction explaining how to initialize book creation for that conversation.

## Goals

- Make book writing accessible through a built-in chat interface.
- Keep the author workflow conversational.
- Support collaborative authoring by multiple participants in the same conversation.
- Provide near-immediate visual feedback after every accepted change.
- Keep backend orchestration in Rust.
- Support a frontend built with Svelte.

## Non-Goals For The Initial Draft

- Locking down an irreversible final internal book file format
- Supporting arbitrary document types beyond books
- Replacing direct editorial review with full autonomous publishing logic
- Locking down a production hosting topology

## Product Principles

- Chat-first authoring: the in-app conversation is the primary write interface.
- Conversation-first ownership: the system is organized around app conversations, not individual user accounts.
- Browser-first reading: the web UI is the primary consumption and review interface.
- Safe orchestration: the agent should operate within clearly scoped project workspaces.
- Traceable changes: every user instruction should be tied to a concrete job or revision.
- Incremental evolution: unresolved product areas should remain explicit rather than implied.
- Explicit initialization: book creation must happen through a Rust-native control command, not implicitly from the first writing request.

## Open Questions

- What exact in-app command and submission patterns should trigger processing?
- Will the rendered frontend show only the latest draft, or revision history too?
- How much autonomy should the agent have versus requiring explicit user commands?
