# Proposal: Autoskills Discovery Support

## Intent

AgentSync's `skill suggest` command currently detects only 7 technologies (Rust, Node/TS, Astro,
GitHub Actions, Docker, Make, Python) using filename-only scanning in a hardcoded Rust enum. The
broader autoskills/skills.sh ecosystem (https://autoskills.sh) supports 46+ technologies with richer
detection strategies (package.json dependency parsing, config file content scanning, Gradle layout
detection, monorepo workspace resolution) and maps them to ~150 curated skills.

This change extends AgentSync's skill discovery to support the full autoskills catalog — natively
integrated into `skill suggest` and `skill suggest --install` — without requiring network access or
runtime dependency on the autoskills CLI.

## Scope

### In Scope

- Refactor `TechnologyId` from a 7-variant Rust enum to a dynamic `TechnologyId(String)` newtype,
  preserving serialization compatibility for existing technology keys
- Introduce `DetectionRules` struct parsed from `toml::Value` in `[technologies.detect]` blocks,
  supporting: `packages`, `package_patterns` (regex), `config_files`, `config_file_content` (with
  `files`, `patterns`, `scan_gradle_layout`)
- New `CatalogDrivenDetector` implementing `RepoDetector` trait — a generic detector that evaluates
  catalog-defined detection rules against repository contents
- Package.json dependency parsing (dependencies, devDependencies, peerDependencies) for `packages`
  and `package_patterns` matching
- Monorepo workspace resolution: pnpm-workspace.yaml, npm/yarn workspaces in package.json
- Web frontend detection by file extension scanning (`.vue`, `.svelte`, `.astro`, etc.)
- Config file content matching with regex patterns
- Gradle layout scanning (`build.gradle`, `build.gradle.kts`, settings files)
- Expand `catalog.v1.toml` from 123 lines to ~900 lines with all ~46 technologies, ~150 skills,
  and ~11 combos ported from autoskills' `skills-map.mjs`
- Adjust `SkillsShProvider.resolve()` to handle `owner/repo/skill-name` format used by autoskills
  skill IDs
- Combo evaluation with `enabled = true` combos (currently all combos are `enabled = false`)
- Comprehensive tests for the new detector, detection rules parsing, and catalog expansion

### Out of Scope

- Agent-specific installation (the `--agent` flag from autoskills) — AgentSync already handles
  multi-agent skill distribution through its own sync engine
- Runtime fetching of catalog from network — all data embedded at compile time, updated with
  releases
- Changes to skill install/uninstall/update mechanics — those work correctly as-is
- Provider catalog overlay changes — the existing merge semantics remain unchanged
- CLI output format changes — JSON contract and human-readable output remain stable

## Approach

Three key architectural decisions:

**1. TechnologyId becomes dynamic (String-based).** The current `TechnologyId` enum forces a Rust
code change for every new technology. Converting to `TechnologyId(String)` newtype lets catalog
entries define arbitrary technology identifiers. The 7 existing keys (`rust`, `node_typescript`,
etc.) remain valid strings. `from_catalog_key()` becomes a simple constructor instead of a match
table. `TechnologyId` remains a raw identifier type; human-friendly names come from catalog-aware
presentation logic when needed. Serialization via `serde` stays snake_case-compatible.

**2. Data-driven detection from catalog.** Instead of hardcoded filename matching in
`FileSystemRepoDetector`, a new `CatalogDrivenDetector` reads structured `[technologies.detect]`
blocks from the TOML catalog and evaluates them generically. The 7 original technology patterns are
migrated into catalog rules, allowing `CatalogDrivenDetector` to become the sole detector once
parity is reached. Detection rules support:

- `config_files`: presence of specific filenames (current behavior, generalized)
- `packages`: exact match in package.json dependencies
- `package_patterns`: regex match against package.json dependency names
- `config_file_content`: scan specific files for regex patterns
- `scan_gradle_layout`: boolean flag for Gradle project structure detection

**3. Everything embedded in catalog.v1.toml.** All ~46 technologies, ~150 skills, and ~11 combos
ported from autoskills' `skills-map.mjs` directly into the embedded TOML catalog. No network calls,
fully deterministic, works offline. Catalog updates ship with AgentSync releases. The existing
provider overlay mechanism can still extend/override entries.

## Affected Areas

| Area                         | Impact   | Description                                                                                                                                                             |
|------------------------------|----------|-------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `src/skills/suggest.rs`      | Modified | `TechnologyId` enum → `TechnologyId(String)` newtype; `DetectionConfidence` unchanged; presentation layers may use catalog names, while the identifier type remains raw |
| `src/skills/detect.rs`       | Modified | New `CatalogDrivenDetector` struct + `RepoDetector` impl; `FileSystemRepoDetector` retired after migrating the legacy 7 technologies into catalog rules                 |
| `src/skills/catalog.rs`      | Modified | Parse `detect` blocks into `DetectionRules`; relax `TechnologyId` validation to accept any string; add detection-rule evaluation helpers                                |
| `src/skills/catalog.v1.toml` | Modified | Expand from 123 → ~900 lines with full autoskills technology/skill/combo data                                                                                           |
| `src/skills/provider.rs`     | Modified | `SkillsShProvider.resolve()` adjusted for `owner/repo/skill-name` skill ID format                                                                                       |
| `src/commands/skill.rs`      | Modified | Adapt display code for dynamic `TechnologyId` (use catalog `name` instead of enum match)                                                                                |
| `tests/`                     | New      | Tests for `CatalogDrivenDetector`, detection rules parsing, package.json parsing, workspace resolution, expanded catalog validation                                     |

## Risks

| Risk                                                                                | Likelihood | Mitigation                                                                                                            |
|-------------------------------------------------------------------------------------|------------|-----------------------------------------------------------------------------------------------------------------------|
| TechnologyId refactor breaks serialization compatibility                            | Medium     | Preserve exact snake_case string values for existing 7 technologies; contract tests validate JSON output unchanged    |
| Large catalog TOML slows parsing                                                    | Low        | TOML parsing is fast even at ~900 lines; `include_str!` embeds at compile time; benchmark if needed                   |
| Regex patterns in detection rules introduce performance issues during repo scanning | Low        | Compile regexes once during catalog load, not per-file; limit scanning depth; reuse existing `IGNORED_DIRS` filtering |
| skills.sh API format changes for `owner/repo/skill-name` resolve                    | Low        | `resolve()` already handles URL construction; skill IDs are embedded, not fetched dynamically                         |
| Package.json parsing edge cases (malformed JSON, missing fields)                    | Medium     | Defensive parsing with `serde_json`; skip malformed files with warning; test with real-world package.json variants    |
| Monorepo workspace resolution adds scan complexity                                  | Low        | Limit workspace glob resolution to top-level config files; don't recursively resolve nested workspaces                |

## Rollback Plan

1. The `TechnologyId` refactor is the riskiest change. If it causes serialization issues, revert to
   enum and add a parallel `DynamicTechnologyId(String)` used only by `CatalogDrivenDetector`,
   mapping back to enum variants for the original 7.
2. If `CatalogDrivenDetector` produces false positives, disable or narrow problematic catalog rules
   via a feature flag or catalog version bump while keeping the older embedded catalog snapshot
   available for rollback.
3. The catalog expansion is additive — reverting to the original 123-line `catalog.v1.toml` restores
   previous behavior with no code changes needed.
4. All changes are behind the existing `skill suggest` command — no impact on `apply`, `clean`,
   `init`, `status`, or `doctor` commands.

## Dependencies

- `regex` crate — already in dependency tree (used by other modules); needed for `package_patterns`
  and `config_file_content` matching
- `serde_json` crate — already in dependency tree; needed for package.json parsing
- autoskills `skills-map.mjs` data — one-time manual port to TOML format; no runtime dependency

## Success Criteria

- [ ] `skill suggest` in a repository with any of the ~46 autoskills-supported technologies detects
  them correctly
- [ ] Existing 7-technology detection produces identical results (backward compatibility)
- [ ] JSON output contract (`detections`, `recommendations`, `summary`) remains unchanged per
  existing spec
- [ ] `skill suggest --install` works with newly discovered skills from expanded catalog
- [ ] Package.json dependency detection works for exact matches and regex patterns
- [ ] Monorepo workspace resolution discovers technologies in workspace packages
- [ ] All existing tests pass without modification (or with minimal adaptation for TechnologyId type
  change)
- [ ] New tests cover: CatalogDrivenDetector, detection rules parsing, package.json parsing, config
  content scanning, Gradle layout detection, workspace resolution
- [ ] Catalog validates successfully with all ~46 technologies, ~150 skills, and ~11 combos
- [ ] No performance regression in `skill suggest` for typical repositories (< 2 seconds)
