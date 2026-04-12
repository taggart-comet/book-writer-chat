# Backend Spec: Messenger And Agent Orchestration

> Status: This is a high-level specification and is read-only by default. It should be changed only with explicit approval from the engineer.

## Purpose

This part of the Rust backend turns messenger messages into controlled authoring jobs executed by Codex CLI.

## Core Responsibilities

- receive inbound user messages from supported messenger providers
- normalize provider-specific payloads into a common command format
- ignore conversation messages that do not target the bot
- resolve the target conversation, book, and workspace
- handle control commands directly in Rust when they do not require the agent
- enrich commands with internal prompt context
- invoke Codex CLI safely
- persist job status, outputs, and revision metadata
- send progress and result replies back through the messenger

## Functional Flow

### 1. Inbound Message Intake

The backend must support a provider adapter layer so each messenger integration implements the same internal interface.

Minimum normalized fields:

- `provider`
- `provider_chat_id`
- `message_id`
- `timestamp`
- `raw_text`
- `attachments`
- `mentions_bot`
- `sender_display_name` if available from the provider

For v1, image attachments are typed records. Telegram photos, Telegram image documents, and MAX official image attachments can be downloaded into the book workspace before the agent runs. Unsupported MAX media attachment types are rejected instead of being passed through as authoring context.

The backend should treat `provider_chat_id` as the primary external identity key for a book conversation.

The exact MVP provider trigger rules and fixture expectations are defined in `messenger-provider-contracts.md`.

### 2. Conversation And Session Resolution

For each inbound message, the system should resolve:

- conversation identity
- target book identity
- active conversation or authoring session

There should be no first-class internal user identity requirement for the initial system design.

If no book exists yet for a conversation, the system should require an explicit Rust-native setup command to create it.

If a bot-directed writing request arrives before setup is complete, the backend must not create a book implicitly. It should reply with a concise user-facing setup instruction that explains how to initialize book creation for that conversation.

### 3. Bot Triggering And Command Interpretation

Only messages that explicitly target the bot should trigger processing.

Messages between participants in the conversation that do not target the bot must be ignored.

The backend should distinguish between:

- Rust-native control commands
- agent-routed book-writing commands

Examples of MVP Rust-native control commands:

- initialize a book for the conversation, with optional language selection through `init en` or `init ru`; bare `init` defaults to English
- show book status

These commands must be handled entirely in Rust without LLM or Codex agent involvement.

The first version should treat most inbound messages as natural-language authoring instructions after the configured provider-specific trigger rules and control-command parsing are applied.

Later, the system may distinguish between:

- book-writing commands
- editorial commands
- metadata or publishing commands
- help or support commands

### 4. Prompt Enrichment

Before invoking Codex, the backend should compose a prompt package from:

- the raw user instruction
- system-level writing and safety instructions
- current book context
- recent conversation summary
- execution constraints for file access and output expectations

For books whose manifest language is `en` or `ru`, the prompt package should explicitly tell the agent to communicate with the author and write new manuscript prose in the selected language unless the user explicitly requests quoted text in another language.

This layer is important because messenger input alone is too underspecified and should not directly define raw agent behavior.

### 5. Workspace Resolution

Each book should map to an isolated workspace directory containing:

- manuscript source files
- book metadata
- renderable artifacts
- revision-related files

The backend must ensure the agent operates only within the intended workspace scope.

The workspace root should be a local directory such as `books/` or `books-data/`.

Each conversation/book should have a dedicated folder under that root.

That local books root must be ignored by Git.

### 6. Agent Invocation

The backend launches Codex CLI as a controlled child process.

The execution contract for that launcher is defined in `codex-cli-execution.md`.

This should only happen for agent-routed writing commands, not for Rust-native control commands.

It should define:

- working directory
- environment variables
- prompt/input payload
- execution timeout
- maximum concurrency rules

### 7. Result Handling

On completion, the backend should capture:

- success or failure status
- stdout and stderr summaries
- changed files
- resulting revision identifier
- updated render version or snapshot identifier

### 8. Messenger Reply

The outbound messenger reply should include:

- whether the request succeeded, failed, or is still processing
- a short human-readable summary
- a frontend link to inspect the current draft

For control commands, the reply may instead confirm setup, repository linkage, or validation errors.

For bot-directed writing requests received before book initialization, the reply should explain that no book exists yet for the conversation and instruct the user to run the explicit setup command.

## Internal Modules

Suggested Rust module split:

- `messenger`: provider adapters and message normalization
- `conversations`: conversation lookup and mapping to books
- `sessions`: active authoring session handling
- `commands`: Rust-native command parsing and execution
- `prompting`: prompt construction and templates
- `agent`: Codex CLI process launcher and result parser
- `jobs`: writing job lifecycle and persistence
- `books`: workspace resolution and revision bookkeeping
- `repositories`: GitHub repository linkage and sync metadata
- `notifications`: outbound messenger replies

## Job Lifecycle

Suggested writing job states:

- `received`
- `accepted`
- `running`
- `succeeded`
- `failed`
- `timed_out`
- `cancelled`

## Error Handling Expectations

The backend should distinguish between:

- invalid messenger payloads
- conversation resolution failures
- missing book setup for a conversation
- unsupported commands
- control command validation failures
- workspace resolution failures
- agent execution failures
- rendering update failures

Users should receive concise, non-technical failure messages through the messenger, while internal logs retain the technical details.

## Safety And Isolation Requirements

- One conversation’s book workspace must not leak into another’s.
- Prompt enrichment must avoid accidental inclusion of secrets or unrelated user content.
- Agent execution should be bounded by time, directory scope, and resource limits.
- Messenger payloads must be treated as untrusted input.

## Observability

Minimum observability requirements:

- structured logs with job IDs
- correlation between inbound message, conversation, job, revision, and outbound reply
- basic metrics for job counts, latency, and failure rate

## Open Questions

- Should the user get immediate `processing` acknowledgements before the final result?
- Should multiple inbound messages queue serially per book?
- How should the system summarize Codex output for non-technical users?
