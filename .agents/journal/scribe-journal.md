# Scribe Journal - Documentation Tracking

## Documentation Debt

- [ ] Add more complex examples for `nested-glob` with multi-repo setups.
- [ ] Document internal MCP formatter logic for contributors.
- [ ] Add architecture diagrams for the Rust core.
- [ ] Create a "Known Issues" page for edge cases in symlink creation on Windows.

## 2025-05-20 - CLI Flag Inaccuracies

**Learning:** The `version` command was documented as a standalone subcommand in the CLI reference, but it is actually a root-level flag (`-V, --version`). Additionally, the `--experimental-tui` flag for `agentsync init` was missing from the documentation despite being implemented in the code.
**Action:** Always verify command structure (subcommand vs flag) against `src/main.rs` and the output of `cargo run -- --help`. Ensure all flags for each command are documented.

## Planned Improvements

- [ ] Automate synchronization between `CONTRIBUTING.md` and Starlight docs.
- [ ] Add searchable FAQ section to the website.
- [ ] Include video tutorials for common setup tasks.

## 2026-05-02 - Skill Command Description Drift

**Learning:** The CLI reference documented the `skill` command with implementation details ("from `dallay/agents-skills`, external GitHub repositories, or local sources, with `skills.sh` used as a fallback search mechanism") that don't appear in the actual Clap help text. The source of truth (`src/main.rs:291`) simply says "Manage installable AI agent skills from skills.sh/other providers".
**Action:** CLI command descriptions must match the exact Clap `#[command(about = "...")]` or doc comment text. Implementation details belong in behavior sections, not the command summary. Always verify against `cargo run -- <command> --help` output.

## 2025-05-15 - Catalog-driven Skill Detection

**Learning:** The `agentsync skill suggest` command's technology detection shifted from a small hard-coded set to a data-driven catalog (`src/skills/catalog.v1.toml`) now supporting 73+ technologies. The documentation had drifted significantly (claiming only 7 supported).
**Action:** When documenting "supported" lists that are data-driven, use "N+" terminology and refer to the source-of-truth catalog file to ensure long-term accuracy.

## 2026-05-20 - Implemented Combo Evaluation Documented as Deferred

**Learning:** Both the CLI reference and the Skills guide claimed that active evaluation of multi-technology "combo" entries was deferred. However, Phase 2 of `recommend_skills` in `src/skills/suggest.rs` already implements this logic, providing specific recommendations for combinations like `react-hook-form` + `zod`.
**Action:** Before claiming a feature is "deferred" or "planned," verify the relevant logic phases in the implementation (e.g., Phase 2 evaluation loops).

## 2026-05-21 - Gitignore Management Drift

**Learning:** The configuration reference claimed that "all symlink destination paths" are added to .gitignore. In reality, the logic in `src/config.rs` is more nuanced: it includes `.bak` versions for all literal destinations, expands `module-map` entries into individual patterns, always adds a defensive `.agents/skills/*.bak` pattern, and explicitly *skips* `nested-glob` destinations because they are templates.
**Action:** Document the specific behavior for different sync types and the automatic inclusion of backup patterns to avoid user confusion about why certain files are or aren't ignored.
