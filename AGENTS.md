# AI Agent Instructions

> This document provides critical guidelines and instructions for AI coding agents contributing to this project. Follow these rules strictly to ensure consistency and maintain high standards.

---

## Project Overview

This project is called `agentsync` and automates syncing AI agent configurations across multiple coding assistants. It integrates TypeScript (`npm/`) and Rust (`src/`) for a complete solution and uses `pnpm` for JavaScript dependency management and `Cargo` for Rust packaging.

## Build, Lint, and Test Commands

### JavaScript Workspace
- **Dependencies Installation:**
  ```bash
  pnpm install
  ```

- **Linting:** Ensure ESLint is installed for TypeScript linting.
  ```bash
  pnpm run lint
  ```
  *(Add a `lint` script in `package.json` if it’s missing.)*

- **Testing:**
  ```bash
  pnpm test
  ```
  
- **Run a Single Test:** Append `--` followed by the test file name:
  ```bash
  pnpm test -- src/index.spec.ts
  ```

- **Release Workflow:**
  ```bash
  pnpm run release
  ```
  To test the release process without pushing:
  ```bash
  pnpm run release:dry-run
  ```

### Rust Workspace
- **Dependencies Installation:**
  ```bash
  cargo build
  ```

- **Run Project:**
  ```bash
  cargo run
  ```

- **Rust Tests:**
  ```bash
  cargo test
  ```
  - To run a specific test:
  ```bash
  cargo test test_name
  ```
  *(Replace `test_name` with the function name.)*

- **Build for Release:**
  ```bash
  cargo build --release
  ```

## Code Style Guidelines

### General Principles
- **Be Explicit:** Avoid magic values. Leverage constants for clarity.
- **Comment Meaningfully:** Document intent, not the obvious.
- **Strict Typing:**
  - JavaScript: Avoid `any`.
  - Rust: Prefer `Result<T, E>` for errors.

### TypeScript
- **File Structure:** Place logical components under `src/`. Use index files for surfacing public APIs.
- **Imports:**
  1. Standard library: `fs`, `path`, etc.
  2. External library: `@dallay/agentsync`.
  3. Internal paths:
      ```ts
      import { myFunction } from "./utils/helpers";
      ```
  - Use absolute paths (configure `tsconfig.json` accordingly).
- **Formatting:**
  - Use Prettier (`pnpm add prettier` if missing).
  - Always configure `.prettierrc`.
- **Error Handling:**
  - Use `try/catch` blocks for API calls. Log appropriately.

### Rust
- **Modular Design:** Break down functionality into discrete modules (`src/` and `tests/`).
- **Imports:**
  - Use `crate::` for internal paths.
- **Error Handling:**
  - Always employ `thiserror` for custom errors.
- **Formatting:**
  - Ensure `rustfmt` is installed.
  - Format code using:
    ```bash
    cargo fmt
    ```
- **Testing Style:**
  - Use `#[test]` for unit tests and keep them alongside modules.
  - Leverage `dev-dependencies` (e.g., `tempfile`).

### Naming Conventions
- Variables:
  - `camelCase` in JS.
  - `snake_case` in Rust.
- Constants:
  - `SCREAMING_SNAKE_CASE`.
- Functions:
  - Verb-based, e.g., `getUser()`.
- File Names:
  - JavaScript: `kebab-case.ts`.
  - Rust: `snake_case.rs`.

## Testing Strategy

1. **Rust Testing:**
   - Write unit tests within `tests/` or inline after `#[cfg(test)]`.
   - Integration tests go under `tests/` directory, with end-to-end coverage.
   
2. **JavaScript Testing:**
   - Use `jest` with `ts-jest` for TypeScript coverage.
   - Mock external dependencies where necessary.

3. **CI Enforcement:**
   - Utilize GitHub Actions (`.github/workflows/ci.yml`) to ensure tests pass before merging code.

## Available Skills

| Skill | Description | Path |
|-------|-------------|------|
| `rust` | Rust code analysis, suggestions for async patterns, safe refactorings, and testing. | [.agents/skills/rust/SKILL.md](.agents/skills/rust/SKILL.md) |
| `rust-async-patterns` | Master Rust async programming with Tokio, async traits, and concurrent patterns. | [.agents/skills/rust-async-patterns/SKILL.md](.agents/skills/rust-async-patterns/SKILL.md) |
| `pinned-tag` | Manage "pinned tags" and commit SHAs for GitHub Actions security and reproducibility. | [.agents/skills/pinned-tag/SKILL.md](.agents/skills/pinned-tag/SKILL.md) |

## Guidelines for AI Agents

- **Follow the Folder Structure::** Don’t mix JS and Rust modules.
- **Check Before Proposing Changes:** Rely on explicit requirements, and validate assumptions before suggesting.
- **Update Related Documentation:** If code alters usage patterns, reflect the changes in docs like `README.md`.
- **Respect Code Owners:** For sensitive areas (like `Cargo.toml`), defer significant changes to maintainers.

## Project-Specific Notes
- **Copilot Context:**
  Use available context from `/Users/acosta/Dev/agentsync/.github/copilot-instructions.md`. Ensure comments are meaningful.

---
Collaborate responsibly. Always aim for clear, maintainable, and error-free contributions. ¡Métele candela, asere!

## Recent Changes
- 001-skills-sh-integration: Added Rust 1.85 (Edition 2024)
- 001-skills-sh-integration: Added Rust, minimum toolchain Rust 1.70 (stable). Target CI will use the latest stable + None for core (use Rust std + `reqwest` for HTTP, `serde` + `serde_json` for
- 001-skills-sh-integration: Added [if applicable, e.g., PostgreSQL, CoreData, files or N/A]

## Active Technologies
- Rust 1.85 (Edition 2024) (001-skills-sh-integration)
- Local filesystem (`.agents/skills/` directory and `registry.json`) (001-skills-sh-integration)
