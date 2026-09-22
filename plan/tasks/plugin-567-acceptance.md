# Issue #567 Acceptance Plan

## Goal

Close #567 only after the merged plugin implementation has auditable evidence for clean-clone restore, skills/MCP safety, lifecycle commands, and supported-agent fan-out.

## Current baseline

Merged:
- #596: plugin materialization hardening.
- #597: exact-pin restore, clean-clone apply, offline mode.
- #598: default-deny plugin MCP approval and status inspection.

## Acceptance matrix

| Capability | Evidence | Current test/evidence | Gap |
|---|---|---|---|
| Local fixture add/update/remove | Rust/integration tests | `tests/plugins.rs`, `tests/plugins_cli.rs` | Verify final output/contract |
| Clean clone online apply | Lock + config, empty Git cache | `tests/plugins_acceptance.rs` | PASS with injected locked-SHA fetcher; real remote archive not used by default suite |
| Offline apply | No Git cache and no network | `tests/plugins_acceptance.rs` + CLI smoke | PASS for missing and restored Git snapshot |
| Exact SHA restore | Lock location/revision wins | restore tests + acceptance harness | PASS for locked source identity; private auth remains NOT TESTED |
| Snapshot repair/drift | Invalid cache/apply/restore | unit coverage | Confirm all drift classes in acceptance report |
| ZIP/tree safety | limits, traversal, symlink rejection | existing tests | Record as #575 evidence |
| MCP default-deny/approval | allowlist + status | `tests/plugins_mcp.rs`, CLI tests | Add final fan-out matrix evidence |
| Remove revocation | selection/lock/approval rollback | partial CLI coverage | Explicit rollback failure test if needed |
| Vendor behavior | Claude/Codex discovery/install/enablement | #573 remains open | Must re-run/version evidence before parent closure |
| Private marketplace | GITHUB_TOKEN archive fetch | implementation exists | Needs credentialed/fixture-safe acceptance or explicit NOT TESTED |

## Execution order

1. Run focused/full Rust verification on `origin/main`.
2. Execute black-box CLI scenarios using temporary project and fixture:
   add, list, status, restore, apply, offline apply, update, remove.
3. Inspect generated Claude/Codex/Gemini/OpenCode MCP outputs with approved and pending cases.
4. Run or document vendor spike evidence for #573; distinguish discovery, installation, enablement, and execution.
5. Post an evidence comment on #567 and rebaseline/close #573–#577 only where criteria are actually satisfied.

## Current evidence (2026-09-21)

- `cargo test --all-features`: 621 tests passed.
- `cargo clippy --all-targets --all-features -- -D warnings`: passed.
- `cargo build --all-features`: passed.
- Focused plugin suites: 17 plugin tests, 4 CLI tests, and 1 MCP integration test passed.
- Local fixture smoke: `plugin add`, `plugin status --json`, and `apply --offline` passed for a local source.
- `tests/plugins_acceptance.rs`: two black-box-style Git clean-clone scenarios pass with a controlled fetcher: online apply restores the locked source identity; missing-cache offline apply fails; explicit restore enables offline apply; status/remove behavior is verified.
- Codex CLI available at `0.147.0`; Claude Code available at `2.1.267`.
- Codex exposes `plugin add/list/marketplace/remove`; Claude exposes `plugin install/enable/disable/marketplace`. Vendor activation/cache behavior still needs isolated HOME/CODEX_HOME evidence.

## Current blockers

- Git-backed clean-clone black-box run with empty cache is not yet recorded.
- Private GitHub repository/auth path is not tested with credentials.
- #573 still lacks the full versioned discovery/install/enablement/cache/execute matrix.
- #577 still lacks final acceptance evidence and should not be closed yet.

## Completion rule

Do not close #567 while #573 vendor evidence or #577 acceptance evidence is missing. Mark private remote/auth behavior as NOT TESTED with a visible follow-up if credentials are unavailable.
