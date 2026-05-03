# book-writer-chat

`book-writer-chat` is for people who have a book in their head but not the time, confidence, or patience to wrestle it onto a blank page alone. Maybe it is your mom finally writing down the family stories. Maybe it is someone with good ideas and rusty language. The point is simple: talk to the AI in a clean messenger, ask it to draft or reshape the book, and immediately see the result in a readable preview.

Under the hood, the book stays as real text files, not trapped inside some mysterious blob, which means you can keep editing it, preserve it properly, and later turn it into something printable through Amazon print-on-demand or any other publishing path you like.

## How do I deploy it?

1. You should have a `linux/amd64` virtual machine.
2. Point your domain at that server.
3. Install Caddy on the Ubuntu server with `sudo apt-get update && sudo apt-get install -y caddy`, and make sure the server can run the Codex CLI, because the authoring flow depends on it.
4. Run `make build-prod`. On `linux/amd64` it builds the app natively and bundles a Docker-built `linux/amd64` Codex CLI. On other machines it uses Docker to produce the same `linux/amd64` bundle. The result is `build/bin/prod/` with the Rust binary, the `codex` binary, the built frontend, the Caddy config, the launch script, the service file, and `.env.example`.
5. Copy `build/bin/prod/` to `/opt/book-writer-chat` on the server.
6. Copy `.env.example` to `.env` in `/opt/book-writer-chat` and fill in the real values. In particular:
   - `FRONTEND_BASE_URL` must be the full public HTTPS origin, for example `https://books.example.com`
   - `CADDY_SITE_ADDRESS` must be the bare public hostname, for example `books.example.com`
   - `CODEX_CLI_PATH` should point at the bundled binary, for example `/opt/book-writer-chat/codex`
   - do not set `CADDY_SITE_ADDRESS` to `:80` or to a URL with `http://` or `https://`, because that disables automatic TLS provisioning
7. Install the included `book-writer-chat.service` into `systemd` and start it.
    ```shell
    sudo cp /opt/book-writer-chat/book-writer-chat.service /etc/systemd/system/book-writer-chat.service
    sudo systemctl daemon-reload
    sudo systemctl enable --now book-writer-chat
    sudo systemctl status book-writer-chat
    ```
8. Let Caddy handle HTTPS certificates automatically. With a real hostname in `CADDY_SITE_ADDRESS`, Caddy will serve HTTPS and redirect HTTP to HTTPS automatically.

That is the deployment model.

One machine, one folder, one startup script, automatic TLS, persistent book data.

No container runtime in production. No certificate babysitting. No unnecessary ceremony. No need to explain to your mom why writing her memoir suddenly requires a platform team.

> **Human Note**: the project, as all of them now, is vibecoded, but it works - I tested it myself (with codex v0.128.0)
