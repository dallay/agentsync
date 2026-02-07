# Contributing to AgentSync

Thank you for your interest in contributing to AgentSync! To maintain high code quality and consistency, please follow these guidelines.

## Development Workflow

1. **Fork the repository** and create a feature branch from `main`.
2. **Implement your changes**.
   - If it's core logic, it goes in `src/` (Rust).
   - If it's related to the NPM package, it goes in `npm/agentsync/`.
   - If it's documentation, it goes in `website/docs/src/content/docs/`.
3. **Verify your changes**:
   - **Full Suite**: Run `make verify-all` from the root. This runs all tests, linters, and builds the documentation.
   - For Rust: `make rust-test`, `cargo fmt`, and `cargo clippy`.
   - For TypeScript: `make js-test`, `make js-build` and `make fmt`.
4. **Open a Pull Request** with a clear description of the changes and why they are needed.

## Code Style

### Rust

- Follow the official Rust style guide.
- Use `cargo fmt` before committing.
- Prefer explicit error handling with `Result<T, E>`.
- Write unit tests for new functionality.

### TypeScript

- Use strict typing. **No `any` allowed.**
- Follow the existing kebab-case naming convention for files.

## Commit Messages

We follow the [Conventional Commits](https://www.conventionalcommits.org/) specification:

- `feat: ...` for new features.
- `fix: ...` for bug fixes.
- `docs: ...` for documentation changes.
- `refactor: ...` for code changes that neither fix a bug nor add a feature.

**Example**: `feat: add support for new AI assistant targets`

## Pull Request Process

1. Ensure the CI pipeline passes and all tests succeed locally.
2. Include a clear description of the change, why it is needed, and any migration or compatibility notes.
3. Add or update tests and documentation where appropriate.
4. Link any related issues in the PR description.
5. Once approved by a maintainer, a reviewer will merge the PR.

## Getting Help

If you have questions, feel free to open a GitHub Issue or start a Discussion.
