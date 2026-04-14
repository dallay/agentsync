# Design: Fix sync-type-aware status for symlink-contents targets

## Technical Approach

This change keeps `agentsync apply` semantics intact and fixes the mismatch in `agentsync status`. The implementation should replace the current destination-path-only status model with a sync-type-aware validator that builds expectations per configured target, validates the observed filesystem shape against that expectation, and then renders both human-readable and JSON output from the same normalized result.

The narrowest implementation path is:

1. keep `Linker` as the source of truth for source resolution and path expectations
2. add a richer status validation model in `src/commands/status.rs`
3. teach that model how to validate `symlink`, `symlink-contents`, and existing expanded targets (`module-map`) according to their actual apply behavior
4. update docs and wizard-facing copy so product language matches the new validation model

This maps directly to the approved proposal and the new `core-sync-engine` / `documentation` deltas: `symlink-contents` targets must be validated as managed destination directories whose child entries are the managed symlinks, including the valid-empty-source case.

## Architecture Decisions

### Decision: Validate status per target contract, not per raw destination path

**Choice**: Introduce an internal target-level validation model that first determines the configured sync contract, then validates the destination according to that contract.

**Alternatives considered**: Keep the current `StatusEntry` shape and bolt on more booleans; special-case only empty `symlink-contents` directories in the existing `entry_is_problematic()` logic.

**Rationale**: The bug exists because the current model assumes every destination is a single symlink. A small special case would fix one symptom but keep the wrong abstraction. A target-level contract model scales cleanly to `symlink`, `symlink-contents`, `module-map`, and any future sync-type-specific diagnostics.

### Decision: Reuse linker/source-resolution logic for expected child discovery

**Choice**: Add a small reusable expectation helper in `src/linker.rs` (or a status-focused helper beside it) that derives the expected managed entries for `symlink-contents`, including pattern filtering and AGENTS compression behavior.

**Alternatives considered**: Reimplement source scanning independently inside `status.rs`; inspect destination contents only and infer expectations from what already exists.

**Rationale**: `status` must reflect apply semantics, not invent a second rules engine. Reusing linker behavior avoids drift around pattern filtering, compressed `AGENTS.md`, and source existence rules.

### Decision: Keep JSON output target-oriented and additive

**Choice**: Preserve the current top-level “array of target entries” contract, but extend each entry with sync-type-aware fields instead of replacing the output shape entirely.

**Alternatives considered**: Keep JSON unchanged and only fix human output; introduce a new nested JSON document with a breaking schema.

**Rationale**: The proposal explicitly flags output compatibility risk. Additive fields let CI and downstream tooling keep consuming the existing array while gaining the detail needed to distinguish a directory-container target from a single-symlink target.

### Decision: Treat skills-mode hints as secondary annotations, not validation outcomes

**Choice**: Run sync-type validation first, then attach recognized hints from `detect_skills_mode_mismatch()` only when the target is otherwise healthy.

**Alternatives considered**: Fold hints into the main drift engine; keep hints on the old destination-path entry model.

**Rationale**: The existing product behavior intentionally keeps recognized layout hints non-fatal. That distinction should remain clear after the status model becomes richer.

## Data Flow

### Status validation pipeline

```text
agentsync.toml
   │
   ▼
collect configured targets
   │
   ▼
build validation expectation per target
   │
   ├── symlink           -> expected destination symlink target
   ├── symlink-contents  -> expected destination directory + expected child symlinks
   └── module-map        -> expanded per-mapping symlink expectations
   │
   ▼
inspect filesystem state
   │
   ▼
produce normalized validation result
   │
   ├── render human text output
   └── serialize JSON output
```

### Sequence diagram: `symlink-contents` target status

```text
run_status()
  -> collect_status_targets()
  -> build_target_expectation(target)
       -> Linker helper resolves source directory rules
       -> expected child entries derived from source contents + pattern
  -> validate_target(expectation)
       -> inspect destination path type
       -> if directory, validate each expected child entry
       -> collect missing/wrong-type/wrong-target issues
  -> collect_status_hints()
  -> render_status_result()
```

### Validation model for `symlink-contents`

For `type = "symlink-contents"`, the validator should use this contract:

1. Resolve the configured source directory.
2. If the source directory does not exist, keep current missing-source semantics.
3. If the source directory exists, derive the eligible child-entry set using the same rules as apply:
   - direct children only
   - optional `pattern` filter
   - AGENTS compression handling when enabled
4. Validate the destination path as a container:
   - missing destination directory => drift
   - destination exists but is not a directory => drift
   - destination exists as a directory => validate expected child entries inside it
5. For each expected child entry:
   - missing child => drift
   - child exists but is not a symlink => drift
   - child is a symlink but points elsewhere => drift
6. If the expected child-entry set is empty:
   - the source directory is still valid
   - an existing empty destination directory is valid with respect to managed-child presence
   - do not emit “missing” or “not a symlink” solely because the target has zero children

The validator does **not** need to redesign cleanup or apply. It only needs to mirror their existing contract accurately.

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `src/commands/status.rs` | Modify | Replace the current generic destination-entry collector with a sync-type-aware target validator, extend JSON/text rendering, and keep exit-code behavior driven by normalized issues. |
| `src/commands/status_tests.rs` | Modify | Add regression tests for empty `symlink-contents` sources, missing expected child entries, wrong child types/targets, and JSON classification for directory-container targets. |
| `src/linker.rs` | Modify | Expose or factor a helper that derives expected `symlink-contents` child entries using the same rules as apply (`pattern`, source existence, compressed AGENTS behavior). |
| `src/init.rs` | Modify | Tighten wizard/layout summary wording so `.agents/commands/` is described as canonical and destination directories are described as populated containers, not destination symlinks. |
| `README.md` | Modify | Fix lingering `command` vs `commands` examples and document sync-type-aware status semantics for commands destinations. |
| `website/docs/src/content/docs/reference/configuration.mdx` | Modify | Clarify canonical `.agents/commands/`, agent-specific command destinations, and the valid-empty-container behavior for `symlink-contents`. |
| `website/docs/src/content/docs/reference/cli.mdx` | Modify | Update `status` docs to describe per-sync-type validation, directory-container checks, and empty-source semantics. |
| `openspec/changes/fix-symlink-contents-status-model/state.yaml` | Modify | Mark design complete and point the workflow to `tasks`. |

## Interfaces / Contracts

The current `StatusEntry` contract is too narrow for `symlink-contents`. The implementation should introduce an internal normalized result that can still serialize to the existing array style.

```rust
enum StatusValidationKind {
    Symlink,
    SymlinkContents,
    ModuleMap,
}

enum DestinationKind {
    Missing,
    Symlink,
    Directory,
    File,
    Other,
}

enum StatusIssueKind {
    MissingDestination,
    InvalidDestinationType,
    MissingExpectedChild,
    ChildNotSymlink,
    IncorrectLinkTarget,
    MissingExpectedSource,
}

struct StatusIssue {
    kind: StatusIssueKind,
    path: String,
    expected: Option<String>,
    actual: Option<String>,
}

struct StatusEntry {
    destination: String,
    sync_type: String,
    destination_kind: String,
    exists: bool,
    is_symlink: bool,
    points_to: Option<String>,
    expected_source: Option<String>,
    issues: Vec<StatusIssue>,
    managed_children: Option<Vec<StatusChildEntry>>,
}

struct StatusChildEntry {
    path: String,
    exists: bool,
    is_symlink: bool,
    points_to: Option<String>,
    expected_source: String,
}
```

Contract notes:

- For `symlink`, the existing fields still map naturally.
- For `symlink-contents`, `destination` refers to the container directory, while `managed_children` and `issues` explain the child-level validation.
- For `module-map`, the existing expansion-per-mapping behavior may remain if that is the narrowest change; the new contract still allows those entries to serialize consistently.
- Human output should be rendered from `issues`, not from raw booleans.

## JSON / Text Output Considerations

### Human-readable output

Human output should stay compact but become sync-type-aware:

- `symlink`: keep the existing `OK`, `Incorrect link`, and `Missing` style messages.
- `symlink-contents`:
  - success: identify the destination as a managed directory container
  - empty valid case: report as OK, optionally note `0 managed entries expected`
  - drift: report the destination problem first, then child-level reasons (`missing child`, `not a symlink`, `wrong target`)

Example shape:

```text
✔ OK: .claude/commands (symlink-contents container, 0 managed entries expected)
✗ Drift: .claude/commands missing managed child review.md
✗ Drift: .claude/commands/review.md exists but is not a symlink
```

### JSON output

The JSON output should remain machine-readable and stable enough for current consumers:

- keep the top-level array
- retain `destination`, `exists`, `is_symlink`, `points_to`, `expected_source`
- add sync-type-aware fields such as `sync_type`, `destination_kind`, and `issues`
- for `symlink-contents`, include child-level detail so empty-valid containers and real drift are distinguishable

Exit code behavior should remain unchanged: any entry with one or more blocking issues returns exit code `1`, regardless of human or JSON mode.

## Regression Test Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | `symlink` validation still behaves exactly as before | Keep existing `entry_is_problematic`-style assertions or migrate them to the new validator with the same expectations for missing, correct, and wrong-target symlinks. |
| Unit | Empty `symlink-contents` source directory is valid | Add a tempdir fixture where `.agents/commands/` exists but is empty, destination exists as a real directory, and assert no problems are reported. |
| Unit | Missing expected child is drift | Build a `symlink-contents` fixture with two source files and only one destination symlink; assert child-level missing-entry issue is emitted. |
| Unit | Wrong child type or wrong child target is drift | Cover regular-file child, directory child, and stale symlink child cases for a managed destination directory. |
| Unit | Invalid destination type for `symlink-contents` is drift | Pre-create the destination as a file or symlink and assert the validator reports invalid container type. |
| Integration | `collect_status_entries` / replacement collector reflects actual apply semantics | Sync a real fixture with `linker.sync()`, then assert the status model reports the target as healthy for both populated and empty containers. |
| Regression | Skills mode mismatch hint stays non-fatal | Preserve existing hint tests so recognized `symlink` vs `symlink-contents` skills warnings remain advisory, not failures. |
| Regression | JSON output distinguishes empty-valid containers from drift | Serialize status results and assert new fields show `sync_type = symlink-contents`, destination kind `directory`, and empty `issues` for the valid-empty case. |

The important regression boundary is that this change must remove the false-positive “not a symlink” result for valid `symlink-contents` containers without hiding real child-entry drift.

## Documentation Update Plan

Documentation updates should be handled as one acceptance set so naming and semantics do not drift apart again.

1. **README**
   - replace incorrect `.agents/command/` examples with `.agents/commands/`
   - correct post-apply tree examples so command destinations are shown as populated directories containing symlinked entries
   - add one short note that `status` validates `symlink-contents` targets as directories of managed child symlinks

2. **Configuration reference**
   - keep `.agents/commands/` as the canonical source directory
   - list `.claude/commands/`, `.gemini/commands/`, and `.opencode/command/` as destination examples
   - update `symlink-contents` examples to use `source = "commands"`
   - explain the empty-source / empty-destination valid case explicitly

3. **CLI reference**
   - update the `status` field descriptions so they are no longer described as a pure destination-symlink check
   - document target-level semantics for `symlink` vs `symlink-contents`
   - note that JSON now includes enough detail to distinguish directory-container validation from single-symlink validation

4. **Wizard/layout copy**
   - ensure generated layout summaries describe commands destinations as populated by `agentsync apply`
   - avoid wording that implies `.claude/commands` or `.gemini/commands` should themselves be symlinks

## Migration / Rollout

No migration required. This is a status-model and documentation correction layered on top of existing apply behavior. Rollout is immediate once the validator, output updates, and regression tests land.

## Open Questions

- [ ] Whether `module-map` should remain serialized as one entry per mapping during this change, or whether the new target-level status model should normalize it too. The narrowest path is to leave current per-mapping expansion in place.
