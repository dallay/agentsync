## Exploration: wizard-preserve-existing-skill-links

### Current State

`TargetConfig.sync_type` in `src/config.rs` is the only data-model switch between directory-level
`symlink` and per-entry `symlink-contents`. `Linker::process_target()` in `src/linker.rs` dispatches
`Symlink` targets to `create_symlink()` and `SymlinkContents` targets to
`create_symlinks_for_contents()`.

On current `main`, `src/init.rs::DEFAULT_CONFIG` already defaults skills targets to
`type = "symlink"` for Claude, Codex, Gemini, and OpenCode (recent work:
`feat: symlink entire skills directory instead of individual skill entries (#261)`). The wizard does
**not** ask about skills mode; it always writes `DEFAULT_CONFIG` unchanged. Wizard migration only
copies discovered skill directory contents into `.agents/skills/` and optionally backs up originals.

Existing-state preservation is only implicit and only when config already matches layout:
`create_symlink()` skips an existing destination symlink if `read_link(dest)` equals the expected
relative source. There is no equivalent semantic check for `symlink-contents` vs an existing
directory symlink. In a reproduced mismatch (`type = "symlink-contents"` + existing
`.claude/skills -> ../.agents/skills`), `agentsync status` reports `OK`, `agentsync doctor` reports
no issues, but `agentsync apply` backs up and replaces child entries inside the symlinked
directory (e.g. `.claude/skills/foo -> foo`), effectively mutating the source tree and creating
churn.

There is no post-init validation step. `status` only checks whether the destination symlink resolves
to the expected source path, not whether the configured sync mode matches the on-disk shape.
`doctor` validates missing sources, destination conflicts, and unmanaged Claude skills, but not
mode-semantic mismatches. Docs are also stale:
`website/docs/src/content/docs/reference/configuration.mdx` and `guides/skills.mdx` still describe
skills as `symlink-contents`.

### Affected Areas

- `src/config.rs` — `SyncType`, `TargetConfig`, and helper expansion logic define the mode contract.
- `src/init.rs` — `DEFAULT_CONFIG`, `scan_agent_files()`, and `init_wizard()` own wizard UX,
  migration, and generated config.
- `src/linker.rs` — `process_target()`, `create_symlink()`, and `create_symlinks_for_contents()`
  implement the divergent behaviors; current mismatch path causes churn.
- `src/commands/status.rs` — reports only destination/source link correctness, not mode semantics.
- `src/commands/doctor.rs` — best place for pre-apply warnings about mode-only mismatches.
- `src/main.rs` — only needed if adding `--preserve-existing-links` CLI surface.
- `website/docs/src/content/docs/reference/configuration.mdx` — outdated example and target-type
  explanation.
- `website/docs/src/content/docs/reference/cli.mdx` and `guides/skills.mdx` — wizard/docs need
  explicit skills-mode guidance.
- `tests/test_agent_adoption.rs`, `src/commands/doctor_tests.rs`, `src/commands/status_tests.rs`,
  `src/init.rs` tests, and `src/linker.rs` tests — coverage for mismatch detection, prompt defaults,
  and preservation.

### Approaches

1. **Wizard-first safe defaults** — add a skills-strategy prompt (or computed confirmation) per
   relevant agent, infer a recommended default from existing destination shape, write config
   accordingly, and run a wizard-only validation summary before exit.
    - Pros: Solves the actual init confusion; keeps fix localized to init UX + validation; aligns
      with the issue scope.
    - Cons: Does not help users who already have mismatched configs unless they rerun wizard or
      doctor.
    - Effort: Medium

2. **Diagnostics-first** — leave wizard generation mostly unchanged, but teach `doctor`/`status` to
   detect “same source, wrong mode semantics” and warn before apply churn.
    - Pros: Helps all existing repos; lower UX risk; easier to test deterministically.
    - Cons: Still lets wizard create surprising configs; user learns only after init.
    - Effort: Medium

3. **Combined minimal fix** — wizard prompt + inferred default + doctor warning + docs, with an
   optional non-interactive preserve flag only if it cleanly reuses the same inference.
    - Pros: Covers both new-init and existing-config confusion without changing sync semantics.
    - Cons: Slightly broader than a pure wizard change.
    - Effort: Medium

### Recommendation

Use **Approach 3**, but keep scope tight:

1. **Wizard mode awareness** in `src/init.rs`: for each generated skills target, detect existing
   destination shape and choose a recommended default (`symlink` when the destination is already a
   directory symlink to the canonical source; otherwise current default). Make the prompt explicit
   about `symlink` vs `symlink-contents` semantics.
2. **Post-init wizard validation summary**: after writing config, inspect each skills target and
   print a warning when on-disk shape differs only by mode semantics before the user runs `apply`.
3. **Doctor warning** in `src/commands/doctor.rs`: add a reusable shape check so existing repos can
   see the mismatch without rerunning wizard. This should specifically catch
   directory-symlink-to-source + `symlink-contents` configs, and optionally the inverse shape when
   safe to recognize.
4. **Documentation updates** for CLI/config/skills guidance.

Defer `status` changes unless they can reuse the same diagnosis without complicating the normal
OK/problem output, and defer `--preserve-existing-links` unless there is a concrete non-interactive
use case beyond wizard convenience. The interactive wizard can already provide migration-safe
defaults once inference exists.

### Risks

- **False positives on inverse detection**: recognizing a real directory full of per-entry symlinks
  as an intentional `symlink-contents` layout is easy; recognizing arbitrary user-managed
  directories as “mode mismatch” is riskier.
- **Prompt creep**: prompting per agent can get noisy in repos with many skill targets;
  recommendation should be compact and maybe skipped when no skills target exists in generated
  config.
- **Current main already changed defaults**: because `DEFAULT_CONFIG` now uses `symlink`, some of
  the original issue is partially fixed; the proposal should focus on explicitness and mismatch
  detection, not re-changing defaults.
- **Docs drift**: docs currently still say skills use `symlink-contents`; implementation and docs
  must be updated together.

### Ready for Proposal

Yes — recommended proposal scope: “make init/doctor mode-aware for skills targets and document
preservation behavior.” Keep it as a single change. It is coherent, directly addresses the
confusion, and avoids broader sync-engine changes.
