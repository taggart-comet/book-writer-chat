# Build Action 080: Observability, Hardening, And Release Readiness

## Goal

Complete the MVP by adding operational visibility, failure clarity, safety boundaries, and release-level verification for the full system.

## Sequencing Note

When an agent implements this action, it should assume all earlier numbered build actions have already been completed and may be relied on as existing project context.

## Scope

This action should implement:

- structured logs with correlation across message, conversation, job, revision, and reply
- baseline metrics for volume, latency, and failure rate
- error classification and user-facing message rules
- rate limiting or basic abuse protection
- secure link validation behavior for the reader surface
- deployment readiness validation on top of the combined Docker and Caddy runtime

## Required Decisions

- choose the logging and metrics approach for MVP
- define which reader links are opaque, signed, authenticated, or otherwise protected
- define minimum production configuration requirements

## Acceptance Criteria

- Each significant request path emits traceable structured logs with stable identifiers.
- Failure categories are distinguishable in backend behavior and user-facing messaging.
- The backend enforces basic safety limits for inbound messaging and agent execution.
- Reader links are not treated as uncontrolled public identifiers unless explicitly specified.
- The system exposes readiness information and operational diagnostics suitable for the combined Docker and Caddy deployment model.
- The project has a release-ready regression checklist covering backend tests, end-to-end flows, and browser verification.

## Verification

### API Tests

- Add tests for error mapping, link validation behavior, and safety limit enforcement.
- Add tests proving logs and metrics hooks receive the expected identifiers for key flows where practical.

### End-To-End API Tests

- Execute failure-path scenarios including invalid payloads, expired or invalid links, render failures, agent timeouts, and deployment-runtime misconfiguration.
- Assert that user-facing responses stay concise while internal diagnostics remain detailed.

### Frontend Verification

- Use Puppeteer MCP to verify:
  - invalid or expired reader links show the intended error experience
  - recoverable backend errors degrade cleanly in the browser
  - the final reader experience for a valid link still renders correctly after hardening changes
