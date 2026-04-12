# Messenger Provider Contracts

## Purpose

This specification freezes the MVP messenger triggering rules and fixture shapes so adapters can be implemented and verified autonomously.

## MVP Provider Scope

MVP provider coverage is:

- Telegram: real adapter behavior
- MAX: real image attachment normalization and download behavior, plus legacy fixture compatibility

This keeps the provider abstraction honest without blocking implementation on an under-specified second platform integration.

## Normalized Inbound Message Contract

Every provider adapter must normalize inbound messages into this shape:

- `provider`
- `provider_chat_id`
- `message_id`
- `timestamp`
- `raw_text`
- `attachments`
- `mentions_bot`
- `sender_display_name`

For v1 image support, `attachments` contains typed image attachment records rather than raw strings. Each image attachment includes:

- `kind: image`
- provider file identifiers
- optional original filename, MIME type, dimensions, byte size, and caption

Telegram supports real image intake for photos and image documents. Telegram image captions are treated like message text for bot mention and command parsing. The backend uses `TELEGRAM_BOT_TOKEN` only when it needs to call Telegram `getFile` and download an image.

MAX supports real image intake for official `message_created` webhook updates. MAX image attachments have `type: image` and a payload containing `photo_id`, `token`, and `url`; the adapter maps the image URL into the existing provider file identifier field and keeps `photo_id` as the optional unique identifier. The production downloader fetches that URL only while processing an authoring message with image attachments. Unsupported MAX attachment types are rejected.

## Bot Identity

The backend must be configured with a canonical bot handle per provider.

Suggested environment keys:

- `TELEGRAM_BOT_USERNAME`
- `TELEGRAM_BOT_TOKEN`
- `MAX_BOT_HANDLE`
- `MAX_ACCESS_TOKEN`

`MAX_ACCESS_TOKEN` is the MAX Bot API access token used for webhook subscription and other bot API operations. Inbound image download uses the official attachment `payload.url` supplied by MAX in the webhook body.

## Trigger Rules

### Telegram

A message counts as bot-directed when at least one of these is true:

- the message starts with `@<telegram_bot_username>`
- the message starts with `/bookbot`
- the message is a reply to the bot and begins with a natural-language instruction or supported control command

Supported MVP Rust-native control commands:

- `/bookbot init`
- `/bookbot status`

All other bot-directed Telegram messages are treated as authoring requests after control-command parsing.

### MAX

For MVP, the adapter contract uses a simple handle-based trigger rule:

- the message starts with `@<max_bot_handle>`

Supported MVP Rust-native control commands:

- `@<max_bot_handle> init`
- `@<max_bot_handle> status`

All other bot-directed MAX messages are treated as authoring requests after control-command parsing.

## Non-Directed Messages

Messages that do not match the provider trigger rules must be ignored and must not create jobs, sessions, or outbound notifications.

## Canonical Control Command Semantics

- `init`: create the conversation-owned book if it does not already exist
- `status`: return concise current state for the conversation without invoking the agent

Repeated `init` for the same conversation must be idempotent and must not create a second book.

## Fixture Requirements

The repository should include fixture payloads for at least these cases per provider:

- bot-directed `init`
- bot-directed `status`
- bot-directed natural-language authoring request
- non-bot message that must be ignored

## Example Normalized Fixtures

### Telegram `init`

```json
{
  "provider": "telegram",
  "provider_chat_id": "telegram:123456",
  "message_id": "101",
  "timestamp": "2026-04-05T10:00:00Z",
  "raw_text": "/bookbot init",
  "attachments": [],
  "mentions_bot": true,
  "sender_display_name": "Alice"
}
```

### Telegram authoring request

```json
{
  "provider": "telegram",
  "provider_chat_id": "telegram:123456",
  "message_id": "102",
  "timestamp": "2026-04-05T10:02:00Z",
  "raw_text": "@bookbot write an introductory chapter about habit formation for busy parents.",
  "attachments": [],
  "mentions_bot": true,
  "sender_display_name": "Alice"
}
```

### MAX `init`

```json
{
  "provider": "max",
  "provider_chat_id": "max:room-42",
  "message_id": "201",
  "timestamp": "2026-04-05T10:00:00Z",
  "raw_text": "@bookbot init",
  "attachments": [],
  "mentions_bot": true,
  "sender_display_name": "Bob"
}
```

### Ignored message

```json
{
  "provider": "telegram",
  "provider_chat_id": "telegram:123456",
  "message_id": "103",
  "timestamp": "2026-04-05T10:03:00Z",
  "raw_text": "I think the next chapter should mention sleep.",
  "attachments": [],
  "mentions_bot": false,
  "sender_display_name": "Charlie"
}
```
