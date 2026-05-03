# Deployment Specification

> Status: This is a high-level specification and is read-only by default. It should be changed only with explicit approval from the engineer.

## Purpose

This specification defines the high-level deployment model for `book-writer-chat`.

The deployment approach should follow the same general pattern as the reference project:

- `/Users/maksimtaisov/Documents/hse/code/drilling-vis/specs/deployment.md`

The concrete deployment implementation may differ because this project uses Rust and Svelte, but the operational shape should stay similar.

## High-Level Deployment Model

The default production deployment should be a single copied application bundle on one `linux/amd64` machine.

That bundle should include:

- the Rust backend application binary
- the built Svelte frontend assets
- a Caddy configuration file
- a startup script that launches the backend and Caddy together
- a sample `.env` file
- a `systemd` service file

This means deployment should not require separate frontend and backend containers in the default production path.

## Combined Bundle Responsibility

The copied bundle should be responsible for:

- running the Rust backend service
- serving the built frontend through Caddy
- terminating HTTPS certificates through Caddy
- routing API traffic to the backend
- routing browser traffic to the frontend assets

## Caddy Role

Caddy should be used for:

- automatic HTTPS certificate generation
- domain-based serving
- reverse proxying API routes to the Rust backend
- serving or routing frontend content

At a high level, the expected behavior is:

- `/api/*` routes go to the Rust backend
- health and operational endpoints go to the Rust backend
- all other browser routes resolve to the frontend application

## Expected Runtime Shape

The intended runtime shape on the deployed host is:

1. `systemd` starts one launch script from the copied bundle.
2. Caddy listens on the public HTTPS port.
3. The Rust backend listens on an internal application port.
4. The built Svelte frontend is available on disk as static output.
5. Caddy proxies backend routes and serves the frontend for application routes.

## Rust Adaptation Of The Reference Pattern

The reference project uses Python, but this project should apply the same pattern with Rust:

- build the Rust backend binary for `linux/amd64`
- build the Svelte frontend in a frontend build step
- assemble a copy-to-server bundle with the backend binary, built frontend assets, Caddy configuration, startup script, and deployment templates

This keeps the production machine free of build toolchains and avoids a container runtime in the default path.

## Suggested Build Structure

The repository should have a deployment-oriented build area that contains at least:

- a Caddyfile
- a launch script
- a production `.env` example
- a `systemd` service template
- a build script that assembles the production bundle

A likely layout is something like:

```text
build/
  Caddyfile
  run-prod.sh
  .env.production.example
  book-writer-chat.service
  build-prod.sh
```

## Process Model On The Host

Because the deployment target is one copied bundle running multiple application concerns, the runtime must start both:

- Caddy
- the Rust backend

The implementation should use:

- a small startup script
- `systemd` as the long-running process owner

The mechanism should stay operationally simple.

## Environment Configuration

The deployment should support environment-driven configuration for at least:

- public domain
- ACME email for Caddy
- backend bind address or port
- public frontend API URL if needed by the frontend build
- any repository or storage configuration needed by the backend

The final set of environment variables will be specified later, but the deployment model should be based on explicit environment configuration rather than hardcoded values.

## Local Data Considerations

The runtime needs persistent host directories for local book workspaces and related generated data.

That should include at least:

- local book workspace data under the chosen books root
- application state data
- Caddy data required for certificates and state

## Makefile Requirements

The project must include a root `Makefile`.

At minimum, it must provide:

- `make up`
- `make build`

## `make up`

`make up` should run the full application locally in a developer-friendly way.

Its goal is to let a developer start everything needed for local work from one command.

The exact implementation can be chosen later, but it should start or coordinate at least:

- the Rust backend
- the Svelte frontend
- any local routing layer needed for development, if applicable

This command is for local development, not necessarily for production-like Docker execution.

## `make build`

`make build` may remain available for the combined Docker image, but the default production path should use a dedicated production-bundle command such as `make build-prod`.

Its goal is to produce a copy-to-server bundle containing:

- the Rust backend binary
- the built frontend
- Caddy configuration
- the launch script
- deployment templates

## Relationship To The Reference Project

This project should borrow the deployment philosophy from `drilling-vis`, especially:

- a combined deployment artifact
- Caddy as the public edge and router
- an operationally simple runtime entrypoint

It should not blindly copy the Python-specific implementation details.

## Non-Goals For This High-Level Spec

This document does not yet lock down:

- the exact build host or CI environment
- the exact Caddy package source
- the exact host directory layout beyond the required data roots
- the final cloud hosting provider
- the final CI/CD pipeline

## Open Questions

- Should local development use host processes, Docker Compose, or the same combined image?
- What exact port layout should the Rust backend use behind Caddy?
- Should frontend assets be served directly by Caddy or by the backend behind Caddy?
- What persistent data directories must be mounted in production from day one?
