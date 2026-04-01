SHELL := /usr/bin/env bash

# Tools
PNPM := pnpm
CARGO := cargo
RUSTFMT := rustfmt
PRETTIER := $(shell command -v npx >/dev/null 2>&1 && echo "npx prettier" || echo "prettier")

# Root/workspace helpers
JS_WORKSPACE := $(PNPM) --filter agentsync

.PHONY: help all install js-install js-test js-build \
        rust-build rust-test rust-run e2e-test fmt docs-dev docs-build docs-preview \
        agents-sync agents-sync-clean clean verify-all check-all check-rust check-js check-docs check-e2e

help:
	@echo "Makefile for agentsync"
	@echo ""
	@echo "Main targets:"
	@echo "  make all             -> install + build (js + rust)"
	@echo "  make verify-all      -> run all tests, linters, and checks (RUN_E2E=1 for e2e)"
	@echo "  make install         -> install dependencies (pnpm + cargo build deps)"
	@echo "  make js-install      -> pnpm install (workspace root)"
	@echo "  make js-test         -> run JS tests (pnpm test)"
	@echo "  make js-build        -> build JS packages (if 'build' script exists)"
	@echo "  make rust-build      -> cargo build"
	@echo "  make rust-test       -> cargo test"
	@echo "  make e2e-test         -> run E2E tests in docker"
	@echo "  make rust-run        -> cargo run"
	@echo "  make fmt             -> rustfmt + biome (if installed)"
	@echo "  make docs-dev        -> start docs in dev mode"
	@echo "  make docs-build      -> build docs"
	@echo "  make docs-preview    -> preview docs"
	@echo "  make agents-sync     -> pnpm run agents:sync"
	@echo "  make agents-sync-clean -> pnpm run agents:sync:clean"
	@echo "  make clean           -> cleans common artifacts"
	@echo ""
	@echo "Examples:"
	@echo "  make js-test"
	@echo "  make rust-test"

all: install js-build

verify-all: check-all

check-all: check-rust check-js check-docs check-e2e
	@printf "\n✅ All checks passed successfully!\n"

check-rust:
	@printf "\n🦀 Verifying Rust...\n"
	@echo "→ Checking formatting..."
	@$(CARGO) fmt --all -- --check
	@echo "→ Running clippy (strict)..."
	@$(CARGO) clippy --all-targets --all-features -- -D warnings
	@echo "→ Running tests (all features)..."
	@$(CARGO) test --all-features

check-e2e:
ifdef RUN_E2E
	@printf "\n🐳 Running E2E tests...\n"
	@$(MAKE) e2e-test
else
	@printf "\n⏭️  Skipping E2E tests (set RUN_E2E=1 to enable)\n"
endif

check-js:
	@printf "\n📜 Verifying JavaScript/TypeScript...\n"
	@echo "→ Installing dependencies..."
	@$(PNPM) install --silent
	@echo "→ Running Biome check..."
	@$(PNPM) run biome:check
	@echo "→ Typechecking and building NPM package..."
	@$(PNPM) --filter @dallay/agentsync run test
	@$(PNPM) --filter @dallay/agentsync run build

check-docs:
	@printf "\n📚 Verifying Documentation...\n"
	@echo "→ Building docs to check for errors..."
	@$(PNPM) --filter agentsync-docs run build

install: js-install rust-build
	@echo "Installation complete."

# JavaScript / pnpm targets
js-install:
	@echo "Running pnpm install (workspace root)..."
	$(PNPM) install

js-test:
	@echo "Running JS tests..."
	$(JS_WORKSPACE) run test

js-build:
	@echo "Running JS build (workspace scripts if present)..."
	$(JS_WORKSPACE) run --if-present build

# Rust targets
rust-build:
	@echo "Building Rust workspace..."
	$(CARGO) build

rust-test:
	@echo "Running Rust tests..."
	$(CARGO) test --all-features

rust-run:
	@echo "Running Rust project..."
	$(CARGO) run

# E2E Tests
e2e-test:
	@echo "Running E2E tests with Docker Compose..."
	@status=0; \
	docker compose -f tests/e2e/docker-compose.yml build test-runner-ubuntu || status=$$?; \
	if [ $$status -eq 0 ]; then docker compose -f tests/e2e/docker-compose.yml up --no-build --abort-on-container-exit --exit-code-from test-runner-ubuntu test-runner-ubuntu || status=$$?; fi; \
	docker compose -f tests/e2e/docker-compose.yml down --volumes --remove-orphans; \
	exit $$status

# Formatting
fmt:
	@echo "Formatting Rust + JS..."
	@if command -v rustfmt >/dev/null 2>&1; then \
		$(CARGO) fmt; \
	else \
		echo "rustfmt not found; skipping"; \
	fi
	@if $(PNPM) exec biome --version >/dev/null 2>&1; then \
		$(PNPM) exec biome format --write .; \
	else \
		echo "biome not available; skipping"; \
	fi

# Docs
docs-dev:
	@echo "Starting docs (dev)..."
	$(PNPM) run docs:dev

docs-build:
	@echo "Building docs..."
	$(PNPM) run docs:build

docs-preview:
	@echo "Preview docs..."
	$(PNPM) run docs:preview

# Agentsync shortcuts (from package.json)
agents-sync:
	@echo "Running agents:sync..."
	$(PNPM) run agents:sync

agents-sync-clean:
	@echo "Running agents:sync:clean..."
	$(PNPM) run agents:sync:clean

clean:
	@echo "Cleaning artifacts..."
	-$(CARGO) clean
	-$(PNPM) run -w --silent clean 2>/dev/null || true
	@rm -rf target
	@echo "Done."
