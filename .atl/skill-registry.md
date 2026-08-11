# Skill Registry — agentsync

## Project
agentsync — Rust CLI + TypeScript npm wrapper + Astro docs. Syncs AI agent configs via symlinks.

## Compact Rules

### Rust / Clippy
- **Trigger**: Editing any `src/**/*.rs`
- **Rule**: `cargo clippy --all-targets --all-features -- -D warnings` must pass before commit

### Formatting
- **Trigger**: Any Rust file changed
- **Rule**: `cargo fmt --all` before commit

### Testing
- **Trigger**: Any change
- **Rule**: `cargo test --all-features` before PR; E2E tests require `RUN_E2E=1`

### CI Gate (pre-push)
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-features`

## Detected Stack

| Component | Technology |
|---|---|
| Language | Rust (edition 2024, rustc 1.89) |
| CLI | Clap 4.5 |
| HTTP | reqwest 0.13 (blocking feature present) |
| Async runtime | Tokio (rt-multi-thread, macros, fs) |
| TUI | ratatui 0.30 + crossterm |
| Serialization | serde, toml, serde_json, serde_yaml |
| Testing | cargo test, tempfile |
| Linting | rustfmt, clippy (strict -D) |

## Active Skills (project-specific)

- `sdd-*` phases: SDD workflow for durable feature changes
- `brainstorming`: for temporary design discussions
- `verification-before-completion`: before claiming work done
- `writing-plans`: for implementation plans from approved specs
- `systematic-debugging`: for bug investigation
- `codebase-architecture`: for architecture refactors

## Relevant Code Paths

| File | Role |
|---|---|
| `src/main.rs` | CLI entry, subcommand dispatch |
| `src/update_check.rs` | Background version check against crates.io — uses `reqwest::blocking` |
| `src/skills/provider.rs` | Skill resolution via skills.sh API — uses `reqwest::blocking` |
| `tests/test_catalog_integrity.rs` | E2E catalog reachability checks — uses `reqwest::blocking` |
| `Cargo.toml` | reqwest has `"blocking"` feature — must be removed after migration |
