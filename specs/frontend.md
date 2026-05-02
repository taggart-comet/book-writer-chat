# Frontend Specification

> Status: This is a high-level specification and is read-only by default. It should be changed only with explicit approval from the engineer.

## Purpose

This specification defines the high-level frontend direction for the web application that renders the user’s in-progress book after each in-app conversation change.

## Framework Direction

The frontend will be built with Svelte using the SvelteKit and Vite stack from the reference project.

The frontend is responsible for:

- rendering the current book draft in a polished reading interface
- fetching current book and job state from the Rust backend
- presenting loading, updating, and error states clearly
- integrating the reader with the app messenger flow

## Security And Dependency Policy

Frontend dependencies must be controlled strictly.

Rules:

- npm package versions must be pinned strictly, not ranged
- dependency versions must be selected from the approved drilling base project only
- if a required package is not present in the drilling base project, it must not be introduced until explicitly reviewed
- package upgrades must be treated as deliberate specification changes, not incidental implementation details

## Approved Dependency Source

The drilling base project was used to create a checked-in approved package snapshot for this repository.

The canonical approved package list for autonomous implementation lives in:

- `specs/frontend-approved-packages.md`

That snapshot was derived from:

- `/Users/maksimtaisov/Documents/hse/code/drilling-vis/frontend/package.json`

Implementation workflow:

1. use `specs/frontend-approved-packages.md` as the local source of truth
2. copy only those exact pinned versions into this project
3. avoid adding packages that are unnecessary for the MVP
4. treat any change to the approved package set as an explicit specification update

If this project needs a frontend package that is not present in the checked-in approved snapshot, that package is not approved yet.

## Versioning Requirements

Allowed:

- exact pinned versions such as `1.2.3`

Disallowed:

- caret ranges such as `^1.2.3`
- tilde ranges such as `~1.2.3`
- broad ranges such as `>=1.2.3`
- floating tags such as `latest`

## Frontend Scope

At the high level, the frontend should provide:

- a book reading view
- revision freshness/status indicators
- clear empty/loading/error states
- direct access from app-generated reader links

## Responsive Direction

The web UI must be mobile-first.

Requirements:

- design primary layouts for narrow mobile viewports first, then enhance for larger screens
- support both phone-sized and desktop-sized browsers without losing core functionality
- avoid desktop-only interaction assumptions such as hover-only affordances or permanently wide side panels
- keep navigation, reader actions, and messenger actions accessible on touch devices
- treat responsive behavior as part of the product contract, not a later polish task

The frontend should not assume the final internal structure of the book yet. It must be prepared to render a backend-defined content contract that may evolve while the manuscript model is still being specified.

## Selection Reference Tools

The reader must support selecting visible book text and using that selection for authoring feedback.

Required selection actions:

- Copy text: copies the selected rendered text exactly enough for ordinary paste workflows.
- `Упомянуть эти строки`: opens an action menu for sending a manuscript reference into the web messenger flow.

The `Упомянуть эти строки` action must offer exactly two options:

- in a new conversation
- in the latest active conversation

The generated manuscript reference payload must include:

- the Markdown source file path for the selected rendered text
- the selected line and character span in that source file
- the selected rendered text as a quoted excerpt

If the user chooses the latest active conversation option, the frontend must resolve that target using conversation metadata supplied by the backend for the current book.

This is an intentional authoring workflow exception to the frontend’s normal render-oriented boundary. The frontend must still not fetch raw manuscript files directly; it may consume source-reference metadata only when that metadata is emitted by the reader API as part of render output.

## Integration Requirements

The frontend must integrate with the Rust backend for:

- current book metadata
- current rendered book content
- recent writing job state
- link access validation or viewer authorization

The current local-development integration convention is:

- use `PUBLIC_BACKEND_BASE_URL` for browser requests to the Rust backend
- default to same-origin requests when that variable is unset
- provide an example local value of `http://127.0.0.1:3000`

## Open Questions

- Which subset of the currently approved reference-project packages is actually required for the MVP frontend?
- Decide whether rendering is server-side, client-side, or hybrid.
- Decide whether authentication is required from the first frontend version.
