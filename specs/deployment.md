# Deployment Specification

> Status: This is a high-level specification and is read-only by default. It should be changed only with explicit approval from the engineer.

## Purpose

This specification defines the high-level deployment model for `book-writer-chat`.

The deployment approach should follow the same general pattern as the reference project:

- `/Users/maksimtaisov/Documents/hse/code/drilling-vis/specs/deployment.md`

The concrete deployment implementation may differ because this project uses Rust and Svelte, but the operational shape should stay similar.

## High-Level Deployment Model

The application should be deployed as a single Docker image that contains the full runnable application stack.

That combined image should include:

- the Rust backend application
- the built Svelte frontend assets
- Caddy for HTTPS termination and request routing

This means deployment should not require separate frontend and backend containers in the default production path.

## Combined Container Responsibility

The combined container should be responsible for:

- running the Rust backend service
- serving the built frontend
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

The intended runtime shape inside the deployed image is:

1. Caddy listens on the public HTTPS port.
2. The Rust backend listens on an internal application port.
3. The built Svelte frontend is available as static output or equivalent build artifacts.
4. Caddy proxies backend routes and serves the frontend for application routes.

## Rust Adaptation Of The Reference Pattern

The reference project uses Python, but this project should apply the same pattern with Rust:

- use a multi-stage Docker build
- build the Rust backend binary in a builder stage
- build the Svelte frontend in a frontend build stage
- assemble the runtime image with the backend binary, built frontend assets, and Caddy configuration

This keeps the final image smaller and avoids shipping build toolchains in the runtime layer.

## Suggested Build Structure

The exact file paths can be decided later, but the repository should have a deployment-oriented build area that contains at least:

- a combined Dockerfile
- a Caddyfile
- any entrypoint or process-launch script needed to run Caddy and the backend together

A likely layout is something like:

```text
build/
  Dockerfile
  Caddyfile
  entrypoint.sh
```

## Process Model Inside The Container

Because the deployment target is a single image running multiple application concerns, the runtime must start both:

- Caddy
- the Rust backend

The implementation may use:

- a small entrypoint script
- a lightweight process supervisor
- another simple and explicit multi-process startup strategy

The exact mechanism is not yet fixed, but it should stay operationally simple.

## Environment Configuration

The deployment should support environment-driven configuration for at least:

- public domain
- ACME email for Caddy
- backend bind address or port
- public frontend API URL if needed by the frontend build
- any repository or storage configuration needed by the backend

The final set of environment variables will be specified later, but the deployment model should be based on explicit environment configuration rather than hardcoded values.

## Local Data Considerations

The runtime may need mounted or persistent storage for local book workspaces and related generated data.

That should include at least:

- local book workspace data under the chosen books root
- Caddy data required for certificates and state

The exact volume layout will be specified later.

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

`make build` should build the combined Docker image for the full application.

Its goal is to produce the single deployable image containing:

- the Rust backend
- the built frontend
- Caddy configuration and runtime components

## Relationship To The Reference Project

This project should borrow the deployment philosophy from `drilling-vis`, especially:

- a combined deployment artifact
- Caddy as the public edge and router
- an operationally simple runtime entrypoint

It should not blindly copy the Python-specific implementation details.

## Non-Goals For This High-Level Spec

This document does not yet lock down:

- the exact Docker base images
- the exact process supervisor or entrypoint implementation
- the exact volume mounts
- the final cloud registry or hosting provider
- the final CI/CD pipeline

## Open Questions

- Should local development use host processes, Docker Compose, or the same combined image?
- What exact port layout should the Rust backend use behind Caddy?
- Should frontend assets be served directly by Caddy or by the backend behind Caddy?
- What persistent data directories must be mounted in production from day one?
