# AGENTS.md

## Project Context

This project is `book-writer-chat`.

It is a full-stack system with:

- a Rust backend
- a Svelte frontend
- a specification-first workflow under `specs/`

## Communication Constraint

The user communicates with the agent using speech-to-text software.

This means some words, especially technical terms, framework names, package names, and file or tool names, may be transcribed incorrectly even when the user said them correctly.

## Required Agent Behavior

When interpreting user requests:

- double-check unusual technical words before treating them as intentional
- consider whether a strange term is a speech-to-text corruption of a known technical term
- prefer correcting obvious transcription mistakes internally when the intended meaning is clear from context
- ask the user for clarification when the intended term is materially ambiguous
- avoid locking incorrect transcriptions into specs, code, filenames, or architecture decisions

When communicating with book users inside a book conversation:

- assume the user is not technical at all
- keep all replies plain-language, concise, and easy to follow
- never expose implementation details, code details, tool names, command names, prompts, models, file names, file paths, logs, configuration, infrastructure, or debugging information
- never describe progress in software-engineering terms such as patching, refactoring, running commands, editing files, changing configuration, or inspecting logs
- describe work in book-writing terms such as drafting, revising, reorganizing, clarifying, expanding, shortening, or incorporating feedback
- if clarification is needed, ask short questions about the book itself, such as audience, tone, structure, content, or intent
- if something goes wrong, explain it in simple user-facing language without exposing technical causes or internal system behavior
- optimize all user-visible wording for a nontechnical person using the product to write a book, not for an engineer

## Examples Of Likely Speech-To-Text Risk

- framework names
- package names
- CLI tool names
- API names
- file paths
- programming language terminology

## Working Rule

If a technical word looks wrong, do not blindly propagate it. First determine whether it is:

1. an obvious transcription error that can be safely corrected
2. a plausible but ambiguous term that requires user confirmation
3. a genuinely new project-specific term the user intended

## Repository Conventions Agents Must Preserve

- treat `books-data/` as the default local book workspace root unless a spec change explicitly says otherwise
- keep local manuscript workspace data and local env files out of Git
- when changing frontend packages, use only exact pinned versions allowed by `specs/frontend-approved-packages.md`
- preserve the current backend module layout under `src/`:
  `app/` for composition, `messaging/` for inbound bot flows, `authoring/` for job execution flow, `reader/` for signed-reader APIs and HTML, `storage/` for persistence and workspace access, and `core/` for shared config/models
- preserve the current local development contract:
  `make up` starts backend and frontend host processes, and the frontend uses `PUBLIC_BACKEND_BASE_URL` for backend integration
- preserve the current verification and deployment entry points unless a spec change explicitly replaces them:
  `make test`, `make frontend-check`, `make check`, `make build`, and `make deployment-smoke`
