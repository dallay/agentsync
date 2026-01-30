# Implementation Plan: Skills.sh Integration

**Branch**: `001-skills-sh-integration` | **Date**: 2026-01-27 | **Spec**: [specs/001-skills-sh-integration/spec.md](specs/001-skills-sh-integration/spec.md)
**Input**: Feature specification from `specs/001-skills-sh-integration/spec.md`

## Summary

Implement `agentsync skill install <skill-id>` and related commands to fetch and install AI agent skills from the skills.sh ecosystem. This involves adding HTTP capabilities to the CLI, validating manifests against the Agent Skills Specification, and managing a local skill registry in `.agents/skills/`. The implementation focuses solely on supporting skill installation and updates from the skills.sh ecosystem. Multi-provider or pluggable interface is not planned for this phase.

## Technical Context

**Language/Version**: Rust 1.89 (Edition 2024)
**Primary Dependencies**: 
- Existing: `clap`, `serde`, `serde_json`, `anyhow`, `thiserror`
- New: `reqwest` (HTTP client), `tokio` (Async runtime)
**Storage**: Local filesystem (`.agents/skills/` directory and `registry.json`)
**Testing**: `cargo test` for unit tests; `tests/` for integration tests using `tempfile`
**Target Platform**: Cross-platform (Linux, macOS, Windows) via Rust standard library
**Project Type**: CLI tool
**Performance Goals**: 
- CLI startup time < 100ms
- Skill installation < 5s (network dependent, assuming average skill size)
- Registry lookup < 50ms
**Constraints**: 
- No arbitrary code execution during installation
- Must work offline for already installed skills
- Must support proxy configuration (via `reqwest` env vars)
**Scale/Scope**: Support for hundreds of installed skills; manageable registry size (< 1MB)

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **Code Quality (v1.0.1)**: Rust 1.89 ensures strong typing. `rustfmt` and `clippy` will be used for linting/formatting. CI is already configured (assumed).
- **Testing Discipline**: 
  - Unit tests for manifest parsing and registry logic.
  - Integration tests for CLI commands using `assert_cmd` or similar (mocking HTTP).
  - Contract tests: Validation against `agentskills.io` schema examples.
- **Performance**: 
  - Goal: < 100ms startup. 
  - Benchmark: Use `hyperfine` or simple timing in CI for critical paths.
- **UX & Accessibility**: 
  - `clap` provides auto-generated `--help`.
  - JSON output support for all commands (`--json` flag).
  - Error messages will use `thiserror` for clarity and include remediation steps.
- **Observability & Release**: 
  - Logs via `tracing` or `log` crate (if needed) or standard error output.
  - Versioning follows SemVer.
- **Security & Compatibility**: 
  - Validates all downloaded manifests.
  - No executable assets installed without warnings (NEEDS CLARIFICATION on asset types).
  - Rust 1.89 compatibility.

## Project Structure

### Documentation (this feature)

```text
specs/001-skills-sh-integration/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
└── tasks.md             # Phase 2 output
```

### Source Code (repository root)

```text
src/
├── main.rs              # CLI entry point (updated)
├── commands/            # Command implementations
│   └── skill.rs         # New skill command
├── skills/              # New module for skill logic
│   ├── mod.rs
│   ├── install.rs       # Installation logic
│   ├── registry.rs      # Local registry management
│   ├── manifest.rs      # Manifest parsing/validation
└── lib.rs               # Library export

tests/
├── integration/
│   └── skill_install.rs # Integration tests for skill installation
└── fixtures/            # Test data (sample manifests)
```

**Structure Decision**: Modularize skill logic into `src/skills/` to keep `main.rs` clean. Use `src/commands/` for CLI glue code.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| New Async Runtime (`tokio`) | HTTP requests (`reqwest`) require async | Blocking HTTP clients are less efficient and `reqwest` is the standard |
