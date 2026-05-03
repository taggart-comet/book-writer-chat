IMAGE_NAME ?= book-writer-chat:local
PUBLIC_BACKEND_BASE_URL ?= http://127.0.0.1:3000
DEPLOY_TEST_PORT ?= 18080

.PHONY: help up backend frontend-install frontend frontend-check seed-mock-book test check build build-codex build-prod deployment-smoke

help:
	@printf '%s\n' \
		'Available targets:' \
		'  make up                Start backend and frontend dev processes' \
		'  make backend           Run the Rust backend locally' \
		'  make frontend-install  Install frontend dependencies with npm ci' \
		'  make frontend          Run the frontend dev server' \
		'  make seed-mock-book    Seed a local mock book and print a signed reader URL' \
		'  make test              Run the Rust test suite' \
		'  make frontend-check    Run frontend typecheck and production build' \
		'  make check             Run backend and frontend verification together' \
		'  make build             Build the combined linux/amd64 deployment image' \
		'  make build-codex       Build the linux/amd64 Codex CLI bundle binary' \
		'  make build-prod        Build the copy-to-server production bundle' \
		'  make deployment-smoke  Build and smoke test the combined container'

up:
	./build/dev-up.sh

backend:
	cargo run --bin book-writer-chat

frontend-install:
	cd frontend && npm ci

frontend:
	cd frontend && PUBLIC_BACKEND_BASE_URL=$(PUBLIC_BACKEND_BASE_URL) npm run dev -- --host 127.0.0.1

frontend-check:
	cd frontend && npm run check && npm run build

seed-mock-book:
	cargo run --bin seed_mock_book

test:
	cargo test

check: test frontend-check

build:
	docker build -f build/Dockerfile -t $(IMAGE_NAME) .

build-codex:
	./build/build-codex.sh

build-prod:
	./build/build-prod.sh

deployment-smoke:
	./build/deployment-smoke-test.sh
