# Scribe Journal - Documentation Tracking

## Documentation Debt

- [ ] Add more complex examples for `nested-glob` with multi-repo setups.
- [ ] Document internal MCP formatter logic for contributors.
- [ ] Add architecture diagrams for the Rust core.
- [ ] Create a "Known Issues" page for edge cases in symlink creation on Windows.

## Planned Improvements

- [ ] Automate synchronization between `CONTRIBUTING.md` and Starlight docs.
- [ ] Add searchable FAQ section to the website.
- [ ] Include video tutorials for common setup tasks.

## 2025-05-15 - Catalog-driven Skill Detection

**Learning:** The `agentsync skill suggest` command's technology detection shifted from a small hard-coded set to a data-driven catalog (`src/skills/catalog.v1.toml`) now supporting 73+ technologies. The documentation had drifted significantly (claiming only 7 supported).
**Action:** When documenting "supported" lists that are data-driven, use "N+" terminology and refer to the source-of-truth catalog file to ensure long-term accuracy.
