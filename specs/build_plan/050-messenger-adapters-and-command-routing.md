# Build Action 050: Messenger Adapters And Command Routing

## Goal

Implement the messenger-facing backend layer that accepts provider payloads, normalizes them, ignores irrelevant messages, and routes control commands versus authoring requests correctly.

## Sequencing Note

When an agent implements this action, it should assume all earlier numbered build actions have already been completed and may be relied on as existing project context.

## Scope

This action should implement:

- common normalized messenger message contract
- provider adapter abstraction
- initial Telegram real adapter and MAX fixture-backed stub adapter
- bot mention detection
- command parsing for Rust-native control commands
- behavior for bot-directed requests that arrive before explicit setup
- provider payload fixtures defined by `messenger-provider-contracts.md`

## Required Decisions

- choose webhook versus polling strategy for Telegram
- choose whether the MAX stub is exposed only in tests or through a local development entrypoint

## Implementation Decisions

- Telegram uses webhook-style HTTP intake via the backend `/api/messages/telegram` endpoint.
- The MAX stub is exposed through the local development HTTP entrypoint `/api/messages/max`, not only through tests.

## Acceptance Criteria

- Provider-specific payloads are normalized into one internal command format.
- Messages that do not target the bot are ignored.
- Explicit setup commands are handled in Rust without invoking the agent.
- Bot-directed writing requests before setup return a concise instruction instead of implicitly creating a book.
- The routing layer can distinguish control commands from authoring commands.
- Outbound messenger replies use a provider-independent internal notification contract.
- Telegram trigger handling matches `messenger-provider-contracts.md`.
- MAX normalization matches the fixture-driven contract in `messenger-provider-contracts.md`.

## Verification

### API Tests

- Add adapter tests for Telegram and MAX payload normalization.
- Add command-routing tests for:
  - ignored non-bot messages
  - setup command handling
  - status command handling
  - pre-setup writing rejection

### End-To-End API Tests

- Simulate inbound provider payloads through HTTP or adapter entrypoints and assert normalized handling, persistence effects, and outbound reply payloads.

### Frontend Verification

- Not required in this step except keeping previously implemented reader flows intact.
