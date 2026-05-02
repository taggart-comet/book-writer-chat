# Build Action 041: Web Authentication

## Goal

Add the minimal authenticated web boundary required for the in-app messenger and related book-management APIs.

## Sequencing Note

When an agent implements this action, it should assume build actions `010`, `030`, and `040` already exist and may be relied on.

This action should be completed before any web messenger book-management or conversation endpoints are exposed.

## Scope

This action should implement:

- backend login endpoint for a single operator account
- JWT issuance and verification
- environment-driven credential loading
- protection for web messenger API routes
- frontend login flow and token reuse
- clear unauthenticated and expired-session behavior

This step should not yet create books, create conversations, or read Codex session logs.

## Required Decisions

- choose JWT lifetime and refresh strategy for MVP
- choose where the frontend stores the JWT for the current session
- choose how protected route failures redirect or recover in the UI

## Acceptance Criteria

- `POST /api/auth/login` validates credentials from environment variables rather than source-code constants.
- Successful login returns a signed JWT.
- Invalid credentials return `401 Unauthorized`.
- Protected web messenger endpoints reject missing, invalid, or expired tokens with `401 Unauthorized`.
- The frontend can authenticate once and make at least one protected follow-up request successfully.
- The implementation does not introduce multi-user account management.

## Verification

### API Tests

- Add tests for successful login, failed login, invalid token, expired token, and missing token.
- Add tests proving runtime configuration fails clearly when required auth environment variables are absent.

### End-To-End API Tests

- Boot the backend in test mode with fixture credentials, log in through the public endpoint, and use the returned token on a protected placeholder endpoint.

### Frontend Verification

- Open the login page in Chromium, submit valid credentials, confirm the app transitions into the authenticated shell, and capture a screenshot.
- Repeat with invalid credentials and confirm the UI shows a concise failure state.
