# Tasks: CLI Output Extraction and Canonical Agent Documentation

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | 300–450 |
| 400-line budget risk | Medium |
| Chained PRs recommended | No |
| Suggested split | Single PR: extraction, registry/docs validation, CI and regressions |
| Delivery strategy | ask-on-risk |
| Chain strategy | single-pr |

Decision needed before apply: Yes
Chained PRs recommended: No
Chain strategy: single-pr
400-line budget risk: Medium

### Suggested Work Units

| Unit | Goal | Likely PR | Notes |
|------|------|-----------|-------|
| 1 | Output boundary and exact renderer tests | PR 1 | Base: trunk; tests first, then `src/output.rs` extraction |
| 2 | Typed MCP metadata, governed docs, and drift CI | PR 1 | Depends on Unit 1; verify with focused cargo tests and CI config review |

## Phase 1: TDD Contracts and Foundation

- [x] 1.1 Add failing exact renderer tests in `src/output.rs`/its test module for apply, clean, summary, color, blank lines, and `print_lines`; verify with `cargo test output`.
- [x] 1.2 Add failing integration coverage in new `tests/mcp_documentation.rs` for eight native agents, unique IDs, markers, Claude Desktop global notes, and stale/missing rows; verify with `cargo test --test mcp_documentation`.
- [x] 1.3 Add failing MCP metadata unit tests in `src/mcp.rs` for non-empty fields, shared Copilot/VS Code destination, and native-only membership; verify with `cargo test mcp`.

## Phase 2: Output Extraction

- [x] 2.1 Move pure apply/clean/init renderers, banner emission, and `print_lines` from `src/main.rs` to `src/output.rs`, preserving `Vec<String>` ordering and `HumanFormatter` behavior; depend on 1.1; run `cargo test output`.
- [x] 2.2 Update `src/main.rs` imports/call sites to use output APIs while leaving orchestration, `src/init.rs` progress, and `commands/status.rs` JSON ownership unchanged; run `cargo test --all-features`.

## Phase 3: Canonical Metadata and Documentation

- [x] 3.1 Add typed native-MCP documentation metadata/accessor to `src/mcp.rs` without changing generation, aliases, defaults, filters, or OS path resolution; depend on 1.3; run `cargo test mcp`.
- [x] 3.2 Replace native-MCP blocks in `README.md`, `npm/agentsync/README.md`, `website/docs/src/content/docs/guides/mcp.mdx`, and `openspec/specs/mcp-generation/spec.md` with exact marked fragments; include Claude Desktop and remove manual sync guidance.
- [x] 3.3 Implement deterministic marker extraction/rendering and byte-for-byte comparison in `tests/mcp_documentation.rs`, including duplicate/stale rows outside governed fragments; depend on 1.2 and 3.1; run `cargo test --test mcp_documentation`.
- [x] 3.4 Strengthen the verifier-required drift regression to reject abbreviated native-MCP rows outside markers and remove the stale duplicate README row; run focused and full Rust checks.

## Phase 4: CI and Regression Verification

- [x] 4.1 Add a focused `cargo test --test mcp_documentation` step/job to `.github/workflows/ci.yml` before the full matrix; verify YAML and command consistency.
- [x] 4.2 Run regression checks for representative apply/clean dry-run output, status JSON, and MCP generation: `cargo test --all-features` and `cargo test --test mcp_documentation`.
- [x] 4.3 Run `cargo fmt --all -- --check` and `cargo clippy --all-targets --all-features -- -D warnings`; confirm no unrelated output, MCP, or configurable-agent behavior changes.
