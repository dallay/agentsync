# Skill: rust-basic

Description
-----------
Skill for Rust-related tasks: code analysis, suggestions for async patterns, safe refactorings, creating and running tests, and configuration recommendations (Cargo.toml, rustfmt, clippy).

Purpose
--------
Provide reproducible instructions and steps so an automated agent can make changes, review code, and propose improvements to Rust modules in this repository.

Triggers
-----------------------
- When the user asks to "improve/optimize/refactor Rust code".
- When the user requests adding tests/unit tests for a Rust module.
- When compatibility with `rustfmt`, `clippy`, or changes to `Cargo.toml` are requested.
- When asked to create async patterns or review usages of `tokio`/`async-std`.

Capabilities
-----------
- Generate/update unit and integration tests.
- Add/update `thiserror` / `anyhow` error-handling patterns.
- Recommend and apply formatting with `rustfmt` and linting with `clippy`.
- Detect and correct async antipatterns (blocking calls, unnecessary spawn, .await out of context).
- Propose and apply changes to `Cargo.toml` (features, dev-dependencies, edition, rust-version).

Limitations
------------
- Do not perform destructive changes without explicit user confirmation.
- Do not modify files outside `src/`, `tests/`, `Cargo.toml`, and related `README` files without permission.
- Avoid changes that require CI/CD updates without authorization.

Preconditions
--------------
- The repository should build locally (`cargo build`) before major changes.
- `cargo`, `rustfmt`, and `clippy` should be available in the environment where the agent operates (if execution is required).

Expected Output
---------------
Depending on the command, the skill should return:
- A diff or patch with the suggested changes (preferred). E.g., a `patch` or instructions for `git apply`.
- Commands to run for verification: `cargo test`, `cargo fmt -- --check`, `cargo clippy -- -D warnings`.
- A brief explanation of why the change was proposed and associated risks.

Suggested Steps for Common Tasks
----------------------------------
1. Add a unit test for a new function or edge case:
   - Create `tests/<module>_tests.rs` or add `#[cfg(test)] mod tests { ... }` in the module.
   - Use `tempfile` or mocks when interacting with FS or network.
   - Run `cargo test <test_name>` to execute just that test.

2. Refactor error handling:
   - Introduce `thiserror::Error` for public errors that need introspection.
   - Use `anyhow::Context` in integration or CLI functions for better error messages.

3. Formatting and linting:
   - Run `cargo fmt` and `cargo clippy -- -D warnings`.
   - If clippy requires stylistic changes, return a PR with automatic fixes when they are safe.

4. Async review:
   - Search for problematic patterns: `.block_on()` in non-async code, redundant use of `tokio::spawn`.
   - Suggest `async fn` and propagate `.await` appropriately through the call stack.

Commit Policy
-------------------
- Do not create commits without explicit user permission.
- When creating a commit: use a concise, reason-focused message (e.g., `fix(rust): handle None in parse_config to avoid panic`).
- Include test commands or `cargo test -- <name>` in the commit body when applicable.

Example Interaction
----------------------
User: "Add tests for src/config.rs and fix the dangerous unwrap"
Skill:
1. Locate `src/config.rs`.
2. Add `#[cfg(test)]` with cases covering None and Err.
3. Replace `unwrap()` with `?` or `ok_or_else` as appropriate and add handling with `thiserror` if it is a public API.
4. Propose a diff and commands to run: `cargo test -- tests::config_parses`.

Security and Review Notes
----------------------------
- Avoid introducing credentials or absolute paths in changes.
- Verify that dependencies added to `Cargo.toml` have compatible licenses and are minimal.

When to Invoke This Skill
-------------------------
- When the task is purely Rust-related or involves significant changes to the Rust CLI/library in the repo.
- Do not use for TypeScript/Node-only tasks unless they require coordination with Rust binaries.

---

Keep this concise and actionable. If you want me to automatically create a PR with the changes, tell me the base branch and whether to use conventional-commits for the commit message.