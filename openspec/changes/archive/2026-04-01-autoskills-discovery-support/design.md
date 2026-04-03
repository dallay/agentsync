# Design: Autoskills Discovery Support

## Technical Approach

Extend AgentSync's `skill suggest` pipeline from 7 hardcoded technologies to ~46 catalog-driven
technologies by: (1) converting `TechnologyId` from a closed enum to an open `String` newtype, (2)
introducing a `CatalogDrivenDetector` that evaluates structured `DetectionRules` parsed from the
TOML catalog, and (3) expanding `catalog.v1.toml` with the full autoskills dataset. The
`SkillsShProvider.resolve()` gains a deterministic path for `owner/repo/skill-name` IDs embedded in
the catalog.

This maps directly to the proposal's three architectural pillars. The existing
`FileSystemRepoDetector` is retired — its 7 technology detection patterns are migrated into catalog
`detect` blocks, making `CatalogDrivenDetector` the sole detector.

## Architecture Decisions

### Decision: TechnologyId as newtype String (not enum)

**Choice**: Replace `pub enum TechnologyId { Rust, NodeTypeScript, ... }` with
`pub struct TechnologyId(String)` implementing `Deref<Target=str>`, `Eq`, `Ord`, `Hash`,
`Serialize`, `Deserialize` (as plain string). Provide `&str` constants for the original 7 IDs (
`TechnologyId::RUST = "rust"`, etc.).

**Alternatives considered**:

- Keep enum, add `Dynamic(String)` variant — requires matching on `Dynamic` everywhere, two code
  paths
- Use `Cow<'static, str>` — unnecessary complexity; all IDs are owned strings from catalog parsing

**Rationale**: The catalog defines arbitrary technology IDs. An enum forces a code change per new
technology, defeating the data-driven goal. A newtype string is transparent to serde (serializes as
the inner string), trivially constructable, and the `&str` constants preserve ergonomics for the 7
known IDs. The `BTreeMap<TechnologyId, ...>` in `ResolvedSkillCatalog` continues to work because
`Ord` delegates to `String::cmp`.

**Impact on existing code**:

- `from_catalog_key()` → removed; `TechnologyId::new(id)` replaces it (infallible)
- `as_human_label()` → removed from `TechnologyId`; callers use
  `catalog.get_technology(id).map(|t| t.name.as_str())` or fall back to `id.as_ref()`
- `Display` impl → delegates to the inner string; human-friendly names are explicitly a
  presentation-layer concern and come from catalog lookups at display sites when desired
- `Copy` trait → lost (String is not Copy); all existing `Copy` uses are few and convert to
  `.clone()` or borrows
- `binary_search` in `SkillSuggestion::add_match` → works because `TechnologyId: Ord`
- Serde `#[serde(rename_all = "snake_case")]` → removed; the inner string IS the serialized form,
  preserving JSON contract (`"rust"`, `"node_typescript"`, etc.)

### Decision: DetectionRules as typed struct (not raw toml::Value)

**Choice**: Parse `[technologies.detect]` blocks into a `DetectionRules` struct at catalog load
time, stored in `CatalogTechnologyEntry.detect` as `Option<DetectionRules>`.

```rust
#[derive(Debug, Clone, Default, Deserialize)]
pub struct DetectionRules {
    pub packages: Option<Vec<String>>,
    pub package_patterns: Option<Vec<String>>,
    pub config_files: Option<Vec<String>>,
    pub config_file_content: Option<ConfigFileContentRules>,
    pub file_extensions: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConfigFileContentRules {
    pub files: Option<Vec<String>>,
    pub patterns: Vec<String>,
    pub scan_gradle_layout: Option<bool>,
}
```

**Alternatives considered**:

- Keep `toml::Value` and interpret at detection time — no compile-time validation, repeated parsing
  per detect call
- Trait-based detection strategies per rule type — over-engineered for what is essentially
  data-driven pattern matching

**Rationale**: Parsing once at catalog load catches malformed rules early (startup fail-fast for
embedded catalog, warning-skip for provider overlays). The struct is `Deserialize`-compatible with
the TOML inline table syntax. Regex patterns in `package_patterns` and
`config_file_content.patterns` are compiled once during `CatalogDrivenDetector` construction and
cached.

### Decision: Single CatalogDrivenDetector replaces FileSystemRepoDetector

**Choice**: Migrate all 7 existing technology detection patterns into catalog `detect` blocks and
use only `CatalogDrivenDetector`. Remove `FileSystemRepoDetector` entirely.

**Alternatives considered**:

- Keep both detectors, merge results with deduplication — dual code paths, subtle
  ordering/confidence conflicts
- Keep `FileSystemRepoDetector` as fallback for technologies without `detect` rules — unnecessary if
  all 7 get `detect` blocks

**Rationale**: The 7 existing detections map cleanly to `config_files` rules:

- Rust: `config_files = ["Cargo.toml"]`
- Node/TS: `config_files = ["package.json", "tsconfig.json"]`
- Astro: `config_files = ["astro.config.mjs", "astro.config.ts", "astro.config.js"]`
- GitHub Actions: `config_files = [".github/workflows/*.yml", ".github/workflows/*.yaml"]` (glob
  support within config_files)
- Docker:
  `config_files = ["Dockerfile", "compose.yml", "compose.yaml", "docker-compose.yml", "docker-compose.yaml"]`
- Make: `config_files = ["Makefile", "GNUmakefile"]`
- Python: `config_files = ["pyproject.toml", "uv.lock", "poetry.lock", "requirements.txt"]`

One detector, one code path, fully data-driven. The `RepoDetector` trait remains for testability.

**Clarification**: Backward compatibility is defined in terms of observable `skill suggest` behavior
for
the original 7 technologies, not preservation of the retired `FileSystemRepoDetector` type in the
runtime architecture.

### Decision: Deterministic provider resolve for catalog skill IDs

**Choice**: When `SkillsShProvider.resolve(id)` receives an ID containing at least two `/`
separators (e.g., `antfu/skills/vitest`), extract `owner/repo` directly and construct the download
URL without a network search call. Fall back to the existing search API for IDs without `/`.

**Alternatives considered**:

- Always use search API — slower, requires network, fragile for catalog-embedded IDs
- Embed download URLs directly in the catalog — bloats TOML, harder to maintain

**Rationale**: Catalog skill IDs from autoskills already encode the `owner/repo/skill-name` path.
Deterministic URL construction (
`https://github.com/{owner}/{repo}/archive/HEAD.zip#skills/{skill-name}`) eliminates the search
round-trip for catalog-driven installs. The heuristic for `skills/` prefix in the subpath already
exists in `provider.rs` (lines 128–139) and is reused.

### Decision: Confidence assignment strategy

**Choice**:

- `config_files` match at project root → `High`
- `config_files` match in subdirectory → `Medium` (nested first-party) or `Low` (incidental path)
- `packages` exact match in root `package.json` → `High`
- `packages` match in workspace `package.json` → `Medium`
- `package_patterns` regex match → same as `packages` (location-based)
- `config_file_content` match → `Medium` (requires content inspection, slightly less certain)
- `file_extensions` match → `Medium`

**Alternatives considered**:

- Flat `High` for everything — loses the signal that helps `min_confidence` filtering
- Per-rule confidence overrides in TOML — over-engineered for the current use case

**Rationale**: Matches the existing `detection_confidence()` logic in `detect.rs` which assigns
confidence based on file depth and incidental path membership. The catalog's `min_confidence` per
technology already filters low-confidence noise.

## Data Flow

```
    skill suggest
         │
         ▼
    load_catalog(provider)
         │
         ▼
    ResolvedSkillCatalog
    (skills, technologies w/ DetectionRules, combos)
         │
         ▼
    CatalogDrivenDetector::new(&catalog)
    ├── compile regex patterns (package_patterns, config_file_content)
    └── cache compiled regexes
         │
         ▼
    CatalogDrivenDetector::detect(project_root)
         │
         ├─── Phase 1: Collect package.json dependencies
         │    ├── Read root package.json → deps, devDeps, peerDeps
         │    ├── Resolve workspaces (pnpm-workspace.yaml or package.json.workspaces)
         │    └── Read each workspace package.json → merge all dep names
         │
         ├─── Phase 2: Evaluate detection rules per technology
         │    ├── packages → exact match against collected deps
         │    ├── package_patterns → regex match against collected deps
         │    ├── config_files → file/glob existence check
         │    ├── config_file_content → read files, search patterns
         │    └── file_extensions → walkdir scan (max depth 3, reuse IGNORED_DIRS)
         │
         └─── Phase 3: Emit TechnologyDetection per matched technology
              └── confidence assigned per rule type + file location
         │
         ▼
    recommend_skills(&catalog, &detections)
         │
         ▼
    SuggestResponse (detections + recommendations + summary)
```

## File Changes

| File                         | Action | Description                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
|------------------------------|--------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `src/skills/suggest.rs`      | Modify | Replace `TechnologyId` enum (~26 lines) with `TechnologyId(String)` newtype (~50 lines). Add `&str` constants. Remove `from_catalog_key()`, `as_human_label()`. Update `Display` impl. Remove `Copy` derive. Adjust `SkillSuggestion::add_match` to use catalog name lookup. Update `render_human` methods to accept catalog ref or use `technology.as_ref()` for display. (~80 lines changed)                                                                                                                                                                                                                      |
| `src/skills/detect.rs`       | Modify | Remove `FileSystemRepoDetector` and all helper functions (`match_marker`, `is_docker_marker`, `is_python_marker`, etc.). Add `DetectionRules`, `ConfigFileContentRules` structs. Add `CatalogDrivenDetector` struct with `RepoDetector` impl. Add `PackageJsonDeps` parser (read `dependencies`, `devDependencies`, `peerDependencies`). Add workspace resolver (`resolve_workspaces`). Keep `IGNORED_DIRS`, `INCIDENTAL_DIRS`, `should_ignore_entry`, `is_incidental_path`, `detection_confidence` (generalized). Add `CompiledDetectionRules` with cached `Regex` instances. (~250 new lines, ~120 removed lines) |
| `src/skills/catalog.rs`      | Modify | Change `CatalogTechnologyEntry.detect` from `Option<toml::Value>` to `Option<DetectionRules>`. In `normalize_technology_entry()`, remove `TechnologyId::from_catalog_key()` call — construct `TechnologyId::new(id)` directly. Parse `detect` via `toml::Value::try_into::<DetectionRules>()`. In `normalize_combo_entry()`, same `TechnologyId::new()` change. Update `ResolvedSkillCatalog.technologies` key type (still `BTreeMap<TechnologyId, ...>`, works because `Ord` on String). Add `pub fn technology_name(&self, id: &TechnologyId) -> &str` helper. (~50 lines changed)                                |
| `src/skills/catalog.v1.toml` | Modify | Expand from 123 to ~900 lines. Add ~40 new `[[skills]]` entries, ~39 new `[[technologies]]` entries (each with `[technologies.detect]` inline table), ~10 new `[[combos]]` entries with `enabled = true`. Migrate existing 7 technologies to include `detect` blocks. (~780 new lines)                                                                                                                                                                                                                                                                                                                              |
| `src/skills/provider.rs`     | Modify | In `SkillsShProvider::resolve()`, add early return path: if `id` contains 2+ `/` separators, split into `owner/repo` and `skill-name`, construct URL deterministically without search API call. Keep existing search fallback for IDs without `/`. (~30 lines added)                                                                                                                                                                                                                                                                                                                                                |
| `src/commands/skill.rs`      | Modify | In `SuggestInstallJsonResponse::render_human()` (line 526), replace `detection.technology.as_human_label()` with `detection.technology.as_ref()` (or pass catalog for lookup). In `SuggestInstallProvider`, no changes needed. (~10 lines changed)                                                                                                                                                                                                                                                                                                                                                                  |
| `src/skills/mod.rs`          | Modify | No structural changes; `detect.rs` exports change (`CatalogDrivenDetector` replaces `FileSystemRepoDetector` in public API)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |

## Interfaces / Contracts

### TechnologyId newtype

```rust
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TechnologyId(String);

impl TechnologyId {
    // Original 7 IDs as constants
    pub const RUST: &str = "rust";
    pub const NODE_TYPESCRIPT: &str = "node_typescript";
    pub const ASTRO: &str = "astro";
    pub const GITHUB_ACTIONS: &str = "github_actions";
    pub const DOCKER: &str = "docker";
    pub const MAKE: &str = "make";
    pub const PYTHON: &str = "python";

    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl AsRef<str> for TechnologyId {
    fn as_ref(&self) -> &str { &self.0 }
}

impl fmt::Display for TechnologyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
```

### CatalogDrivenDetector

```rust
pub struct CatalogDrivenDetector {
    rules: Vec<(TechnologyId, CompiledDetectionRules)>,
}

struct CompiledDetectionRules {
    packages: Option<Vec<String>>,
    package_patterns: Option<Vec<regex::Regex>>,
    config_files: Option<Vec<String>>,
    config_file_content: Option<CompiledConfigFileContentRules>,
    file_extensions: Option<Vec<String>>,
}

struct CompiledConfigFileContentRules {
    files: Option<Vec<String>>,
    patterns: Vec<regex::Regex>,
    scan_gradle_layout: bool,
}

impl CatalogDrivenDetector {
    pub fn new(catalog: &ResolvedSkillCatalog) -> Result<Self> { ... }
}

impl RepoDetector for CatalogDrivenDetector {
    fn detect(&self, project_root: &Path) -> Result<Vec<TechnologyDetection>> { ... }
}
```

### PackageJsonDeps (internal)

```rust
struct PackageJsonDeps {
    deps: BTreeSet<String>,
    source_path: PathBuf,
    is_workspace: bool,
}

fn parse_package_json(path: &Path) -> Result<Option<PackageJsonDeps>> { ... }
fn resolve_workspaces(project_root: &Path) -> Result<Vec<PathBuf>> { ... }
```

### Catalog TOML detect block format

```toml
[[technologies]]
id = "vue"
name = "Vue.js"
skills = ["acosta/agent-skills/vue"]
min_confidence = "medium"

[technologies.detect]
packages = ["vue", "nuxt"]
package_patterns = ["^@vue/", "^@nuxt/"]
config_files = ["vue.config.js", "nuxt.config.ts", "nuxt.config.js"]
```

```toml
[[technologies]]
id = "spring"
name = "Spring"
skills = ["acosta/agent-skills/spring"]
min_confidence = "medium"

[technologies.detect.config_file_content]
files = ["build.gradle", "build.gradle.kts", "pom.xml"]
patterns = ["org\\.springframework", "spring-boot"]
scan_gradle_layout = true
```

### JSON output contract (unchanged)

The `SuggestJsonResponse` structure remains identical. `TechnologyId` serializes as a plain string —
existing values (`"rust"`, `"node_typescript"`) are unchanged, new values appear as their catalog
string IDs (e.g., `"vue"`, `"spring"`).

## Testing Strategy

| Layer       | What to Test                                  | Approach                                                                                                                                  |
|-------------|-----------------------------------------------|-------------------------------------------------------------------------------------------------------------------------------------------|
| Unit        | `TechnologyId` serde round-trip               | Verify `serde_json::to_string` / `from_str` produces same string; constants match expected values                                         |
| Unit        | `DetectionRules` deserialization              | Parse representative TOML fragments into `DetectionRules` struct; verify all field combinations                                           |
| Unit        | `PackageJsonDeps` parser                      | Test with valid, malformed, empty, and workspace-containing `package.json` fixtures                                                       |
| Unit        | Workspace resolution                          | Test pnpm-workspace.yaml parsing, package.json workspaces array, glob expansion                                                           |
| Unit        | Regex compilation in `CompiledDetectionRules` | Verify invalid patterns produce errors at construction, valid patterns match expected strings                                             |
| Unit        | Deterministic provider resolve                | Verify `owner/repo/skill-name` produces correct URL; verify fallback for simple IDs                                                       |
| Integration | `CatalogDrivenDetector` with temp repo        | Create `TempDir` with known files (Cargo.toml, package.json with deps, config files); verify correct detections and confidence levels     |
| Integration | Full `SuggestionService.suggest()` pipeline   | Verify end-to-end with expanded catalog against fixture repos; check backward compatibility (same detections for original 7 technologies) |
| Integration | Expanded catalog validation                   | Load `catalog.v1.toml`, verify all entries parse without error, all skill references resolve, all technology IDs are unique               |
| Contract    | JSON output shape                             | Existing contract tests in `tests/contracts/` must pass unchanged; add contract for new technology IDs appearing in output                |

## Migration / Rollout

No data migration required. This is a compile-time change:

1. **TechnologyId refactor** — all code compiles against the new type or fails to build. No runtime
   migration.
2. **Catalog expansion** — additive. Existing 7 technologies get `detect` blocks alongside their
   existing data. No breaking schema change (schema version stays `v1`).
3. **FileSystemRepoDetector removal** — clean deletion once catalog `detect` blocks cover all 7
   technologies. Tests validate equivalence before removal.
4. **Provider overlay compatibility** — provider catalogs using the old 7 technology IDs as strings
   continue to work because `TechnologyId(String)` accepts any string. Provider overlays with new
   technology IDs also work.

## Open Questions

- [x] Are `regex` and `serde_json` already in Cargo.toml? — **Yes**, both present (lines 32, 29)
- [ ] Should `file_extensions` scanning respect a configurable max depth, or is hardcoded depth 3
  sufficient? — Propose depth 3 as default, revisit if users report missing detections in deep
  monorepos
- [ ] Should combo `enabled = true` entries be evaluated in this change, or deferred? — Proposal
  says in-scope; design includes it. Combo evaluation in `recommend_skills` checks if all `requires`
  technologies are detected, then adds combo skills to recommendations
