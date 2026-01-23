SHELL := /usr/bin/env bash

# Herramientas
PNPM := pnpm
CARGO := cargo
RUSTFMT := rustfmt
PRETTIER := $(shell command -v npx >/dev/null 2>&1 && echo "npx prettier" || echo "prettier")

# Root/workspace helpers
JS_WORKSPACE := $(PNPM) --filter agentsync

.PHONY: help all install js-install js-lint js-test js-build js-release \
        rust-build rust-test rust-run fmt docs-dev docs-build docs-preview \
        agents-sync agents-sync-clean clean

help:
	@echo "Makefile para agentsync"
	@echo ""
	@echo "Targets principales:"
	@echo "  make all             -> install + build (js + rust)"
	@echo "  make install         -> instalar dependencias (pnpm + cargo build deps)"
	@echo "  make js-install      -> pnpm install (workspace root)"
	@echo "  make js-lint         -> ejecutar linter (pnpm run lint)"
	@echo "  make js-test         -> ejecutar tests JS (pnpm test)"
	@echo "  make js-build        -> build JS packages (si existe script \"build\")"
	@echo "  make js-release      -> release JS (pnpm run release)"
	@echo "  make rust-build      -> cargo build"
	@echo "  make rust-test       -> cargo test"
	@echo "  make rust-run        -> cargo run"
	@echo "  make fmt             -> rustfmt + prettier (si está instalado)"
	@echo "  make docs-dev        -> iniciar docs en modo dev"
	@echo "  make docs-build      -> build docs"
  	@echo "  make agents-sync     -> pnpm run agents:sync"
  	@echo "  make agents-sync-clean -> pnpm run agents:sync:clean"
	@echo "  make clean           -> limpia artefactos comunes"
	@echo ""
	@echo "Ejemplos:"
	@echo "  make js-test"
	@echo "  make rust-test"

all: install js-build

install: js-install rust-build
	@echo "Instalación completa."

# JavaScript / pnpm targets
js-install:
	@echo "Running pnpm install (workspace root)..."
	$(PNPM) install

js-lint:
	@echo "Running JS linter..."
	$(JS_WORKSPACE) run lint

js-test:
	@echo "Running JS tests..."
	$(PNPM) test

js-build:
	@echo "Running JS build (workspace scripts if present)..."
 	# Intent: prefer a workspace build script; ajusta si tu workspace tiene scripts distintos.
 	$(JS_WORKSPACE) run --if-present build

js-release:
	@echo "Running JS release (semantic-release)..."
	$(PNPM) run release

# Rust targets
rust-build:
	@echo "Building Rust workspace..."
	$(CARGO) build

rust-test:
	@echo "Running Rust tests..."
	$(CARGO) test

rust-run:
	@echo "Running Rust project..."
	$(CARGO) run

# Formatting
fmt:
	@echo "Formateando Rust + JS..."
	@command -v rustfmt >/dev/null 2>&1 && $(CARGO) fmt || echo "rustfmt no encontrado; saltando"
	@command -v npx >/dev/null 2>&1 && npx prettier --write . || echo "prettier/npx no encontrado; saltando"

# Docs
docs-dev:
	@echo "Iniciando docs (dev)..."
	$(PNPM) run docs:dev

docs-build:
	@echo "Building docs..."
	$(PNPM) run docs:build

docs-preview:
	@echo "Preview docs..."
	$(PNPM) run docs:preview

# Agentsync shortcuts (desde package.json)
agents-sync:
	@echo "Running agents:sync..."
	$(PNPM) run agents:sync

agents-sync-clean:
	@echo "Running agents:sync:clean..."
	$(PNPM) run agents:sync:clean

clean:
	@echo "Limpiando artefactos..."
	-$(CARGO) clean
	-$(PNPM) run -w --silent clean 2>/dev/null || true
	@rm -rf target
	@echo "Listo."
