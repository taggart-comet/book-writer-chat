# Codex In The Production Bundle

`make build-codex` builds a Linux `amd64` `codex` binary in Docker and writes it into `build/bin/prod/codex`.

`make build-prod` now includes that same binary automatically, so the server does not need to compile Codex from source.

Set `CODEX_CLI_PATH` in `.env` to the bundled binary path:

```bash
CODEX_CLI_PATH=/opt/book-writer-chat/codex
```

Then run the binary on the server once to authenticate with OpenAI before using the authoring flow.

## Rebuilding Only Codex

If the app bundle is already built and you only need a fresh Codex binary:

```bash
make build-codex
```

Optional overrides:

```bash
CODEX_GIT_REF=main make build-codex
CODEX_GIT_REF=<commit-or-tag> make build-codex
CODEX_GIT_URL=https://github.com/openai/codex.git make build-codex
CODEX_BUILD_JOBS=8 make build-codex
```
