# AgenSync

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What Is AgentSync

Rust CLI + TypeScript npm wrapper + Astro docs site. Syncs AI agent configurations (Claude, Copilot,
Cursor, Gemini, Codex, VS Code, OpenCode) from a single `.agents/` source directory using symbolic
links. Root `AGENTS.md` symlinks here.

## Architecture

**CLI entry point:** `src/main.rs` → Clap subcommands: `apply`, `clean`, `init`, `status`, `doctor`,
`skill`.

**Core flow (apply):**
1. `config.rs` — parse `agentsync.toml` (from project root or `.agents/`) into typed config
2. `linker.rs` — core engine: resolve sources, create/update/remove symlinks per agent+target
3. `mcp.rs` — generate MCP server configs in agent-specific formats (JSON/TOML)
4. `gitignore.rs` — update `.gitignore` managed section with marker-delimited blocks

**Four sync types** (set per target in config):
- `symlink` — single file/dir symlink
- `symlink-contents` — symlink each item inside a source directory
- `nested-glob` — recursively match files, create symlinks using destination template placeholders
  (`{relative_path}`, `{file_name}`, `{stem}`, `{ext}`)
- `module-map` — map source files to specific module directories with convention filenames

**Key modules:**
- `src/skills/` — skill management: install, uninstall, update, suggest, detect from external
  providers
- `src/commands/` — subcommand implementations (doctor, skill, status)
- `src/agent_ids.rs` — agent identity mapping and canonical filenames per agent
- `src/init.rs` — template generation + interactive wizard (`init --wizard`)

## Repo Structure

- `src/` — Rust library/CLI
- `tests/` — Integration, contract, unit, E2E tests
- `npm/agentsync/` — TypeScript wrapper (dispatches to platform binaries)
- `website/docs/` — Astro/Starlight docs
- `docs/` — Symlink to `website/docs/src/content/docs` (edit source)
- `.agents/` — Agent config, skills, `agentsync.toml`

## Environment

- Rust: `1.89+`, edition `2024`
- Node: `24.14.0` (per `.nvmrc`)
- Package manager: `pnpm` only

## Setup

```bash
pnpm install      # Runs setup.js → installs Lefthook + runs agents:sync
cargo build
```

## Build Commands

```bash
cargo build
cargo build --release
cargo check --all-targets --all-features
pnpm --filter agentsync run build   # npm wrapper
pnpm run docs:build
make js-build
make rust-build
```

## Lint/Format

```bash
# Rust
cargo fmt --all
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings

# JS/TS/Astro
pnpm exec biome check .
pnpm exec biome check --write .
pnpm run biome:check
pnpm run biome:fix
make fmt
```

## Test Commands

```bash
cargo test --all-features
make rust-test
pnpm --filter agentsync run test      # TypeScript typecheck only
pnpm --filter agentsync run typecheck
make js-test
make verify-all                       # Full check: fmt, JS build/test, docs, clippy, Rust tests
RUN_E2E=1 make verify-all
make e2e-test                         # Docker-based, slower
```

## Run Single Tests

```bash
# By substring
cargo test test_name_substring

# Standalone integration test file
cargo test --test test_bug

# Named test in standalone file
cargo test --test test_bug test_merge_cleanup_bug

# Named test in all_tests harness
cargo test --test all_tests integration::skill_install::integration_skill_install_fixture

# Library/unit test
cargo test --lib test_extract_managed_entries

# With output preserved
cargo test test_name_substring -- --nocapture
```

- `tests/all_tests.rs` → nested modules like `integration::skill_install::*`
- Standalone: `tests/test_bug.rs`, `tests/test_update_output.rs` (use `--test <stem>`)

## CI/Pre-commit Hooks

- Pre-commit: `cargo fmt --all -- --check`,
  `cargo clippy --all-targets --all-features -- -D warnings`
- Pre-push: `cargo test --all-features`
- CI: `cargo check --all-targets --all-features`, `cargo build --release`, security audit
- PR titles: semantic (enforced by workflow)

## Code Style: Rust

- Format: `.rustfmt.toml` → Unix newlines, 4-space tabs, `max_width = 100`
- Naming: `snake_case` (functions/modules), `PascalCase` (types/enums), `UPPER_SNAKE_CASE` (consts)
- Types: Explicit structs/enums over loose maps; `Path`/`PathBuf` for FS; `BTreeMap`/`BTreeSet` for
  ordering
- Doc comments on public APIs/config structs
- `#[derive(Debug, ...)]` for diagnostics/serde/clap

## Imports & Error Handling (Rust)

- Match surrounding file import order
- Group: local crate → external → `std`
- Errors: `anyhow::Result<T>` at CLI boundaries, `thiserror::Error` for domain enums
- Add context: `.with_context()` or `Context` on FS/network ops
- Propagate with `?`; avoid `unwrap()`/`expect()` in prod paths
- Diagnostics: `tracing::{info,warn,error}!`; User output: `println!` + `colored`

## Code Style: TypeScript/Astro

- Biome rules: tabs, double quotes
- Strict TypeScript; no `any`
- Explicit types on exported APIs
- `Record<string, T>` or named interfaces over loose objects
- Node built-ins: `node:` prefix (e.g., `node:fs`)
- Naming: `camelCase` (vars/functions), `PascalCase` (Astro components)

## Testing Guidelines

- Black-box/CLI tests → `tests/`; narrow unit tests → close to module
- Use `tempfile::TempDir` for FS isolation
- Explicit assertions over snapshots (unless contract)
- Update `tests/contracts/` when changing machine-readable CLI output

## What to Edit

- Rust: `src/**/*.rs`, `tests/**/*.rs`
- npm wrapper: `npm/agentsync/src/index.ts`
- npm build output: `npm/agentsync/lib/` (regenerate)
- Docs: `website/docs/src/**` (validate: `pnpm run docs:build`)

## Generated/Avoid

- `target/`, `website/docs/dist/`, `website/docs/.astro/`
- Fix symlinks via source/setup, not hand-editing

## Agents/Skills

- `pnpm run agents:sync` — Apply after changing agent files
- `pnpm run agents:sync:clean` — Clean re-sync

## Commit/PR

- Conventional Commits: `feat:`, `fix:`, `docs:`, `refactor:`, etc.
- Semantic PR titles (CI-enforced)
- Update tests/docs when behavior/output changes
- PR template: summary, test instructions, environment, checklist

## Agent Workflow

- Smallest relevant change + narrowest validating command
- Rust changes: `cargo test <name>` or `cargo test --all-features`
- npm wrapper: `pnpm --filter agentsync run build` + `pnpm --filter agentsync run test`
- Docs: `pnpm run docs:build`
- Broad changes: `make verify-all` before finishing
