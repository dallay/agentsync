# Tasks: Autoskills Discovery Support

## Phase 1: Foundation (Type System & Data Structures)

- [x] 1.1 Replace `TechnologyId` enum in `src/skills/suggest.rs` with
  `pub struct TechnologyId(String)` newtype. Add `#[serde(transparent)]`, derive
  `Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize`. Add `&str` constants
  for 7 existing IDs. Add `new()`, `AsRef<str>`, `Display` impls. Remove `from_catalog_key()`,
  `as_human_label()`, `Copy` derive.
- [x] 1.2 Update all `TechnologyId` consumers: `catalog.rs` (`normalize_technology_entry` →
  `TechnologyId::new(id)`), `detect.rs` (imports, `FileSystemRepoDetector` match arms),
  `commands/skill.rs` (`as_human_label()` → `as_ref()`). Fix all compilation errors from
  enum→newtype change. Run `cargo check`.
- [x] 1.3 Define `DetectionRules` and `ConfigFileContentRules` structs in `src/skills/detect.rs`
  with `#[derive(Debug, Clone, Default, Deserialize)]`. Fields: `packages`, `package_patterns`,
  `config_files`, `config_file_content`, `file_extensions`.
- [x] 1.4 Change `CatalogTechnologyEntry.detect` from `Option<toml::Value>` to
  `Option<DetectionRules>` in `src/skills/catalog.rs`. Update `ProviderCatalogTechnology.detect` in
  `src/skills/provider.rs` similarly. Add `technology_name(&self, id: &TechnologyId) -> &str` helper
  to `ResolvedSkillCatalog`.

## Phase 2: Core Implementation (Detection Engine)

- [x] 2.1 Add `CompiledDetectionRules` and `CompiledConfigFileContentRules` structs in
  `src/skills/detect.rs`. Compile regex patterns from `package_patterns` and
  `config_file_content.patterns` at construction time. Log warnings and skip technologies with
  invalid regex.
- [x] 2.2 Add `parse_package_json(path) -> Option<BTreeSet<String>>` in `src/skills/detect.rs`.
  Parse `dependencies`, `devDependencies`, `peerDependencies` keys using `serde_json`. Handle
  malformed JSON with warning+skip.
- [x] 2.3 Add `resolve_workspaces(project_root) -> Vec<PathBuf>` in `src/skills/detect.rs`. Support
  pnpm-workspace.yaml (`packages` list), npm/yarn `workspaces` array, yarn object-form
  `workspaces.packages`. Resolve globs relative to project root. No recursive resolution.
- [x] 2.4 Implement `CatalogDrivenDetector` struct with `RepoDetector` trait in
  `src/skills/detect.rs`. Constructor takes `&ResolvedSkillCatalog`, builds compiled rules.
  `detect()` method: (1) collect+merge package.json deps from root+workspaces, (2) evaluate
  per-technology rules in order (packages → package_patterns → config_files → config_file_content →
  file_extensions), (3) short-circuit per technology, (4) assign confidence per rule type.
- [x] 2.5 Add web frontend detection via `file_extensions` rule: walkdir scan with max depth 3,
  reuse `IGNORED_DIRS`, emit `medium` confidence with matched extensions as evidence.
- [x] 2.6 Remove `FileSystemRepoDetector` and its helper functions. Migrate 7 existing technology
  detections into catalog `detect` blocks in `catalog.v1.toml`. Update `SuggestionService` to use
  `CatalogDrivenDetector`.

## Phase 3: Integration (Catalog, Provider, Combos)

- [x] 3.1 Expand `src/skills/catalog.v1.toml` with ~40 new technologies (with `detect` blocks), ~140
  new skills (using `owner/repo/skill-name` format), ~11 combos with `enabled = true`, and
  `web_frontend` technology with `file_extensions` detection.
- [x] 3.2 Update `SkillsShProvider::resolve()` in `src/skills/provider.rs`: when skill ID contains
  2+ `/` separators, extract `owner/repo` and `skill-name`, construct GitHub ZIP URL
  deterministically. Keep search API fallback for simple IDs.
- [x] 3.3 Enable combo evaluation in `recommend_skills()` in `src/skills/catalog.rs`: evaluate
  combos with `enabled = true`, check all `requires` technologies present in detections, add combo
  skills to recommendations with combo name in reason. Deduplicate with technology skills.

## Phase 4: Testing & Validation

- [x] 4.1 Unit tests in `src/skills/detect.rs`: `DetectionRules` deserialization,
  `parse_package_json` (valid/malformed/missing), workspace resolution (pnpm/npm/yarn), regex
  compilation (valid/invalid), web frontend extension scanning, confidence assignment.
- [x] 4.2 Unit tests in `src/skills/suggest.rs`: `TechnologyId` serde round-trip, constants match
  expected values, `Display` output.
- [x] 4.3 Integration test: `CatalogDrivenDetector` against `TempDir` with fixture files (
  Cargo.toml, package.json with deps, config files). Verify correct detections, confidence, and
  short-circuit behavior. Test combo evaluation with enabled/disabled combos.
- [x] 4.4 Catalog validation test: load expanded `catalog.v1.toml`, verify all entries parse, all
  skill refs exist, all technology IDs unique, all combos reference valid technologies.
- [x] 4.5 Run `cargo test --all-features`,
  `cargo clippy --all-targets --all-features -- -D warnings`, `cargo fmt --all -- --check`,
  `make verify-all`. Verify backward compatibility: existing 7 technologies produce same detections
  and JSON contract unchanged.
