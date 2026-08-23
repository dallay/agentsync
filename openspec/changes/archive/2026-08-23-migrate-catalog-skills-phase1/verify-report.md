# Verification Report: migrate-catalog-skills-phase1

## Verdict

**PASS WITH WARNINGS**

The QA remediation is technically compliant and executable. The new acceptance target launches an
external release-like `agentsync` binary, observes JSON/exit-code/filesystem/registry behavior, and
passes the Phase 1 sibling, override, missing-source, companion, and suggestion-root scenarios.
The eight stale provenance hashes are corrected in the current sibling worktree and all ten recorded
hashes validate. This report does not claim user/operator acceptance; `sdd-qa` may proceed.

## Change, mode, and completeness

| Item | Result |
|---|---|
| Change | `migrate-catalog-skills-phase1` |
| Persistence mode | `openspec` |
| Scope verified | QA-F-001 provenance remediation and QA-F-002 external acceptance target |
| Original tasks | 15/15 complete (`1.1`–`3.4`) |
| QA remediation tasks | 4/4 complete (`4.1`–`4.4`) |
| Incomplete tasks | 0 |
| Application/source behavior changes | None in this remediation; no `src/` or catalog diff |
| New verification surface | `Makefile` target and three `tests/acceptance/` scripts |
| Sibling skill-content changes | None; only provenance/wrapper/validator remediation files changed |
| Next phase | `qa` |

The unrelated untracked Clerk, Angular, and TypeScript candidate directories in the local
`agents-skills` checkout were inspected only for exclusion and were not modified or counted as
Phase 1 changes. The three migrated skill directories remain tracked and their committed bytes are
unchanged relative to the supplied `be4570aa...` source revision.

## Exact diff inspected

### `agentsync`

- Tracked diff: `Makefile`, `apply-report.md`, `qa-report.md`, `state.yaml`, and `tasks.md`.
- Untracked additions: `tests/acceptance/phase1_catalog.sh`,
  `tests/acceptance/test_phase1_catalog_harness.sh`, and
  `tests/acceptance/test_phase1_provenance.sh`.
- `git diff --name-only -- src tests/test_catalog_integration.rs tests/unit/provider.rs
  tests/unit/suggest_catalog.rs tests/unit/suggest_install.rs` produced no existing application or
  Phase 1 test-source delta; the only new test files are the acceptance scripts above.

### `agents-skills`

- Tracked diff: `PROVENANCE.md` (the eight refreshed hashes plus validator documentation) and
  `scripts/validate-skills.sh` (the validator invocation).
- Untracked addition: `scripts/validate_provenance.py`.
- `git diff --name-status be4570aa... -- skills/drizzle-orm skills/pydantic skills/sqlalchemy` was
  empty; the unrelated candidate paths remain untracked and untouched.
- `git diff --check` and `git -C ../agents-skills diff --check` passed.

## Build, test, coverage, and validation evidence

All commands below were run during this verification on 2026-08-14.

| Command | Result |
|---|---|
| `cargo check --all-targets --all-features` | PASS |
| `cargo build --release` | PASS; release-like target rebuilt |
| `make acceptance-phase1 AGENTSYNC_BIN=target/release/agentsync AGENTSYNC_SOURCE_REPO=../agents-skills` | PASS; external CLI harness completed all four scenarios |
| `tests/acceptance/test_phase1_catalog_harness.sh` | PASS; missing-binary negative contract rejected the absent target |
| `AGENTSYNC_SOURCE_REPO=../agents-skills tests/acceptance/test_phase1_provenance.sh` | PASS; `[OK] validated materialized hashes` |
| `python3 scripts/validate_provenance.py --root .` in `agents-skills` | PASS; all 10 provenance hash entries validated |
| Pinned `skills-ref validate` for `skills/drizzle-orm`, `skills/pydantic`, and `skills/sqlalchemy` | PASS; each reported `Valid skill` |
| `cargo test --all-features --test test_catalog_integration phase1_bobmatnyc_catalog_entries_install_offline_and_register_local_ids -- --nocapture` | PASS; 1 passed |
| `cargo test --all-features --test all_tests 'unit::provider::' -- --nocapture` | PASS; 16 passed |
| `cargo test --all-features --test all_tests 'unit::suggest_catalog::' -- --nocapture` | PASS; 23 passed |
| `cargo test --all-features --test all_tests 'unit::suggest_install::' -- --nocapture` | PASS; 10 passed |
| `cargo test --all-features commands::skill::tests::direct_catalog_install_resolution_uses_the_sibling_agents_skills_checkout` | PASS; 1 passed |
| `cargo test --all-features commands::skill::tests::suggestion_catalog_install_resolution_uses_the_project_root` | PASS; 1 passed |
| `RUN_E2E=1 cargo test --all-features --test test_catalog_integration every_catalog_skill_installs_successfully -- --ignored --nocapture` | PASS only via the intentional early-return message; no full-catalog health claim |
| `cargo llvm-cov --all-features --test all_tests --test test_catalog_integration --summary-only` | PASS; 123 passed, 2 ignored in `all_tests`; 2 passed, 1 ignored in catalog integration; 34.17% line coverage, no threshold asserted |
| `cargo fmt --all -- --check`, `bash -n tests/acceptance/*.sh`, and `git diff --check` | PASS |

The repository-wide `agents-skills/scripts/validate-skills.sh` was not used as the focused result
because it enumerates the unrelated untracked candidates and its later repository-wide checker has
known Clerk activation-cue failures. Its exact diff was inspected and confirms that it invokes
`validate_provenance.py`; the focused validator was run directly and passed.

## External harness assessment

| Property | Evidence | Judgment |
|---|---|---|
| External | Executes `target/release/agentsync` as a separate process; no Rust module, test helper, or in-process API is called | PASS |
| Observable | Asserts JSON `status`, process failure, stderr text, installed `SKILL.md`/companions, registry keys, source markers, and absence of unintended paths | PASS |
| Isolated | Uses `mktemp`, separate project roots, temporary `HOME`, temporary command CWD, and copies only the three Phase 1 directories; markers are added only to copies | PASS |
| Reproducible | Requires explicit `AGENTSYNC_BIN`/`AGENTSYNC_SOURCE_REPO`, preflights all three source directories, disables update checks, and cleans its temporary root with a trap | PASS |
| Phase 1 scope | Covers direct sibling installs for all three entries, override precedence, missing-source fail-closed behavior, companions, local registry keys, and suggestion project-root/provider/local-ID propagation | PASS |
| Full catalog boundary | Does not enumerate or install the full catalog; the separate ignored test still emits the intentional early-return message | PASS WITH WARNING; not a full-catalog result |

The harness does not attempt interruption/retry behavior or network packet interception, and it does
not pin a source commit itself. Those are limitations for later acceptance/release hardening, not a
failure of the intended Phase 1 remediation target. The supplied `AGENTSYNC_SOURCE_REPO` is checked
by the separate provenance validator before QA handoff.

## Spec compliance matrix

| Requirement / scenario | Implementation and runtime evidence | Status |
|---|---|---|
| Qualified ID reaches resolution | `install_selected_with_reporter()` calls `provider.resolve(&recommendation.provider_skill_id)` and passes `recommendation.skill_id` only to installation; `unit::suggest_install::` passed 10 tests | PASS |
| Provider routing remains qualified | `SkillsShProvider` deterministic test for `dallay/agents-skills/docker-expert` passed within the 16 provider tests | PASS |
| Local ID controls installation state | Focused integration and external harness assert local install folders and `registry.json` keys for all three migrated IDs | PASS |
| Install-all preserves both IDs | Existing `install_all_skips_already_installed_recommendations` runtime coverage uses qualified provider fixtures and local IDs; external suggestion `--install --all` also asserts qualified `pydantic` identity and local result | PASS WITH WARNING; the new external flow intentionally narrows pending installs to the Phase 1 suggestion target rather than fabricating three detections |
| Approved source resolves offline | External CLI installs all three from a temporary sibling fixture; focused Rust integration passes and asserts directory resolution rather than online resolution | PASS |
| Missing source blocks | External CLI with an empty override exits non-zero, reports `refusing external fallback`, and creates no `pydantic` directory; provider negative test also passed | PASS |
| Remap preserves metadata | Catalog boundary test passed 23 tests, including local IDs, titles, summaries, removed old definitions, technology mappings, and retained Wispbit boundary | PASS |
| Complete attributable source is eligible | Current `PROVENANCE.md` records immutable source/license/attribution/companion metadata; all 10 materialized hashes validate and all three pinned `skills-ref` checks pass | PASS |
| Incomplete or unsupported source blocks | Only the approved three entries are locally mapped; Clerk and other unrelated candidates remain outside the catalog remap, as confirmed by exact diff and catalog boundary tests | PASS |
| Approved subset installs and registers | External CLI and focused integration both install every approved entry, verify `SKILL.md`/companions, and verify canonical local registry keys | PASS |
| Incomplete subset fails | Harness preflight rejects a missing source and the empty-source install path fails with an identified local skill and no output directory | PASS |
| Focused success remains scoped | The ignored full-catalog test retains `#[ignore]`, `RUN_E2E`, and the explicit early return; the ignored test passed only after printing the skip message | PASS WITH WARNING; full catalog intentionally untested |

All applicable scenarios have runtime coverage. The install-all row is marked with a warning only to
make its deliberate Phase 1 suggestion-fixture narrowing explicit; it does not indicate a failing
implementation or a missing required remediation behavior.

## Correctness and design coherence

### Correctness

| Finding / area | Evidence | Status |
|---|---|---|
| QA-F-001 stale provenance hashes | Eight changed materialized hashes were refreshed; the new validator parsed and recomputed all 10 entries successfully | Resolved for current remediation worktree |
| QA-F-002 missing acceptance target | External binary harness, negative contract test, and reproducible `make acceptance-phase1` target now exist and pass | Resolved for technical handoff |
| Source isolation | Only `drizzle-orm`, `pydantic`, and `sqlalchemy` are copied by the harness; unrelated untracked candidates were not changed or counted | PASS |
| Production behavior | No `src/`, catalog, registry manifest, full-catalog test, or sibling skill-content change was introduced by remediation | PASS |
| Task completion | Tasks `4.1`–`4.4` are checked and evidence above covers each one | PASS |

### Design coherence

| Design decision | Evidence | Status |
|---|---|---|
| Local sibling/override is the Phase 1 source of truth | Harness proves sibling resolution and `AGENTSYNC_LOCAL_SKILLS_REPO` precedence through the CLI | PASS |
| Qualified local IDs and fail-closed behavior | Catalog remap removes mutable Phase 1 install sources; provider tests and missing-source harness prevent fallback | PASS |
| Preserve install/registry semantics | All three external installs and the suggestion install use local IDs for folders and registry keys while exposing qualified provider IDs | PASS |
| Companions are recursive and visible | Harness and focused integration assert the Drizzle, Pydantic, and SQLAlchemy companion files | PASS |
| No registry redesign/full-catalog claim | Registry manifests are unchanged; full-catalog early return remains and is explicitly reported as not a health result | PASS WITH WARNING |
| Remediation stays outside production behavior | Acceptance and provenance checks are shell/Python tooling only; release build and focused tests pass | PASS |

## Issues

### CRITICAL

None. No test command failed, no task is incomplete, and no applicable Phase 1 behavior lacks runtime
evidence after this remediation.

### WARNING

| Finding | Judge A | Judge B | Severity | Status |
|---|---|---|---|---|
| Refreshed `PROVENANCE.md`, wrapper invocation, and validator are uncommitted in the local sibling worktree; merged commit `be4570aa...` still represents the pre-refresh provenance bytes until this remediation is persisted | ✅ | ✅ | WARNING | Confirmed; current worktree validator passes, archive requires a final committed-byte recheck |
| Full-catalog E2E remains ignored and intentionally early-returns while unrelated external entries remain unresolved | ✅ | ✅ | WARNING | Confirmed and explicitly out of scope; no full-catalog health claim |
| Harness has no interruption/retry or network-interception assertion and accepts an explicitly supplied source path rather than enforcing a Git commit itself | ✅ | ✅ | WARNING | Confirmed limitation; not required for the Phase 1 remediation target |
| `scripts/validate-skills.sh` remains repository-wide and will encounter unrelated candidate/Clerk validation issues in the dirty local sibling checkout | ✅ | ✅ | WARNING | Confirmed environment limitation; migrated directories pass individually and unrelated candidates remain untouched |

### SUGGESTION

- Commit/persist only the intended sibling provenance tooling and refreshed record before archive, then
  rerun the provenance validator against the final merged bytes.
- Keep the Phase 1 harness explicitly scoped; do not convert its passing subset into a full-catalog
  health claim.
- Have `sdd-qa` rerun the capability-driven acceptance report using the documented external target.

## Final handoff

This is technical verification only. The remediation passes with the warnings above, and `sdd-qa`
**may proceed** to perform acceptance QA against the external harness. QA must own the acceptance
verdict and update `qa-report.md`; this phase does not promote the previous `NOT TESTED` QA report to
user/operator acceptance or authorize archive.
