# Domain Model

> Status: This is a high-level specification and is read-only by default. It should be changed only with explicit approval from the engineer.

## Core Entities

### Conversation

Represents a messenger conversation that owns a single book workspace.

Suggested fields:

- `conversation_id`
- `provider`
- `provider_chat_id`
- `title`
- `created_at`
- `status`

### Book

Represents a conversation-owned book project.

Suggested fields:

- `book_id`
- `conversation_id`
- `title`
- `status`
- `workspace_path`
- `created_at`
- `updated_at`

### AuthoringSession

Represents an active context for interpreting a sequence of messenger commands.

Suggested fields:

- `session_id`
- `conversation_id`
- `book_id`
- `status`
- `last_message_at`

### WritingJob

Represents a single backend execution triggered by a bot-directed conversation message.

Suggested fields:

- `job_id`
- `book_id`
- `conversation_id`
- `session_id`
- `source_message_id`
- `status`
- `command_kind`
- `prompt_snapshot`
- `started_at`
- `finished_at`

### Revision

Represents a meaningful book state after a writing job.

Suggested fields:

- `revision_id`
- `book_id`
- `job_id`
- `summary`
- `created_at`
- `render_status`

### RenderSnapshot

Represents the frontend-consumable output for a given revision.

Suggested fields:

- `render_snapshot_id`
- `revision_id`
- `format`
- `storage_location`
- `created_at`

### RepositoryBinding

Represents an optional external source-control binding for a book.

This is the canonical source of truth for repository linkage metadata.

Suggested fields:

- `repository_binding_id`
- `book_id`
- `provider`
- `repository_url`
- `repository_name`
- `status`
- `created_at`
- `updated_at`

## Relationships

- one `Conversation` owns one `Book` in the initial model
- one `Book` can have many `AuthoringSession` records over time
- one `Book` can have many `WritingJob` records
- one `WritingJob` may produce zero or one `Revision`
- one `Revision` may produce one or more `RenderSnapshot` artifacts
- one `Book` may have zero or one active `RepositoryBinding`

## State Considerations

### Book status

Possible values:

- `active`
- `archived`
- `blocked`

### Session status

Possible values:

- `active`
- `idle`
- `closed`

### Revision render status

Possible values:

- `pending`
- `ready`
- `failed`

### Repository binding status

Possible values:

- `unlinked`
- `linked`
- `error`

## Domain Assumptions

- A messenger message is not itself the source of truth; the resulting job and revision are.
- The canonical book state lives in the book workspace and its tracked revision metadata.
- Render outputs are derived artifacts and may be regenerated.
- The primary ownership boundary is the conversation, not the individual user.
- One conversation maps to one book workspace folder.
- Some conversation commands are handled entirely by Rust and do not create an agent writing job.
- External repository linkage lives in `RepositoryBinding`, not duplicated book fields.

## Known Undefined Areas

These will be specified later:

- canonical manuscript file format
- chapter or section schema
- media and image asset model
- publishing/export model
