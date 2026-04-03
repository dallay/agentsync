# Autoskills Discovery Specification

## Purpose

Define how AgentSync extends its technology detection and skill recommendation system from a
hardcoded 7-technology enum to a catalog-driven discovery engine supporting ~46 technologies, ~150
skills, and ~11 combos from the autoskills ecosystem — all embedded at compile time without network
dependency.

## Requirements

### Requirement: Dynamic TechnologyId Newtype

The system MUST represent technology identifiers as a `TechnologyId(String)` newtype struct instead
of a fixed Rust enum.

`TechnologyId` MUST implement `Debug`, `Clone`, `PartialEq`, `Eq`, `PartialOrd`, `Ord`, `Hash`,
`Serialize`, and `Deserialize`.

`TechnologyId` MUST provide named constants for the 7 existing technologies: `RUST`,
`NODE_TYPESCRIPT`, `ASTRO`, `GITHUB_ACTIONS`, `DOCKER`, `MAKE`, and `PYTHON`.

`TechnologyId::new()` MUST accept any string and return a valid `TechnologyId`. The previous
`from_catalog_key()` method MUST be replaced by `TechnologyId::new()`.

`TechnologyId` MUST serialize to its inner string value using `serde`, preserving snake_case
compatibility for existing technology keys (e.g., `"node_typescript"`, `"github_actions"`).

`TechnologyId` MUST behave as a raw identifier type. Its `Display` implementation MAY render the raw
identifier string directly. Human-friendly labels, when needed, MUST come from catalog-aware
presentation logic or helper functions outside the identifier type itself.

#### Scenario: Existing technology key round-trips through serialization

- GIVEN a `TechnologyId` created with value `"node_typescript"`
- WHEN the value is serialized to JSON and deserialized back
- THEN the resulting `TechnologyId` MUST equal the original
- AND the serialized JSON string MUST be `"node_typescript"`

#### Scenario: Arbitrary technology key is accepted

- GIVEN the catalog defines a technology entry with `id = "vue"`
- WHEN `TechnologyId::new("vue")` is called
- THEN a valid `TechnologyId` MUST be returned
- AND it MUST be usable in detection results, catalog lookups, and serialization

#### Scenario: Raw identifier display remains stable

- GIVEN a `TechnologyId` with value `"github_actions"`
- WHEN the identifier is formatted directly
- THEN the rendered text MAY be `"github_actions"`
- AND any human-friendly label such as `"GitHub Actions"` MUST come from catalog-aware presentation
  logic rather than from the identifier type alone

#### Scenario: Named constants match existing serialized values

- GIVEN the constant `TechnologyId::RUST`
- WHEN it is serialized to JSON
- THEN the serialized value MUST be `"rust"`
- AND `TechnologyId::NODE_TYPESCRIPT` MUST serialize to `"node_typescript"`
- AND `TechnologyId::GITHUB_ACTIONS` MUST serialize to `"github_actions"`

---

### Requirement: Detection Rules Schema

The catalog MUST support structured detection rules in `[technologies.detect]` blocks that define
how a technology is detected from repository contents.

Each detection rule block MAY contain any combination of the following fields:

- `packages`: a list of exact package name strings to match against package.json dependencies
  (dependencies, devDependencies, peerDependencies)
- `package_patterns`: a list of regex pattern strings to match against package.json dependency names
- `config_files`: a list of filenames or relative paths whose existence in the repository indicates
  the technology
- `config_file_content.files`: a list of filenames to read content from
- `config_file_content.patterns`: a list of string patterns to search for within those files
- `config_file_content.scan_gradle_layout`: a boolean flag that, when true, causes the detector to
  scan Gradle build files (`build.gradle`, `build.gradle.kts`, `settings.gradle`,
  `settings.gradle.kts`) for the specified patterns

All fields within a detect block MUST be optional. A technology entry with an empty or absent detect
block MUST NOT cause a catalog loading error.

Regex patterns in `package_patterns` and `config_file_content.patterns` MUST be compiled once during
catalog loading and reused across all evaluations.

If a regex pattern is invalid, the system MUST reject that specific technology's detection rules
during catalog loading and MUST log a warning, but MUST NOT fail the entire catalog load.

#### Scenario: Technology with package-based detection

- GIVEN the catalog defines a technology entry with:
  ```toml
  [technologies.detect]
  packages = ["next", "next-auth"]
  ```
- WHEN the repository contains a `package.json` with `"next"` in `dependencies`
- THEN the technology MUST be detected

#### Scenario: Technology with regex package pattern detection

- GIVEN the catalog defines a technology entry with:
  ```toml
  [technologies.detect]
  package_patterns = ["^@angular/"]
  ```
- WHEN the repository contains a `package.json` with `"@angular/core"` in `dependencies`
- THEN the technology MUST be detected
- AND the evidence MUST reference the matched package name

#### Scenario: Technology with config file existence detection

- GIVEN the catalog defines a technology entry with:
  ```toml
  [technologies.detect]
  config_files = ["tailwind.config.js", "tailwind.config.ts", "tailwind.config.mjs"]
  ```
- WHEN the repository root contains `tailwind.config.ts`
- THEN the technology MUST be detected
- AND the evidence MUST reference `tailwind.config.ts`

#### Scenario: Technology with config file content scanning

- GIVEN the catalog defines a technology entry with:
  ```toml
  [technologies.detect.config_file_content]
  files = ["build.gradle", "build.gradle.kts"]
  patterns = ["org.springframework"]
  ```
- WHEN the repository contains `build.gradle.kts` with the line
  `implementation("org.springframework.boot:spring-boot-starter")`
- THEN the technology MUST be detected
- AND the evidence MUST reference the matched file and pattern

#### Scenario: Technology with Gradle layout scanning

- GIVEN the catalog defines a technology entry with:
  ```toml
  [technologies.detect.config_file_content]
  files = []
  patterns = ["com.android"]
  scan_gradle_layout = true
  ```
- WHEN the repository contains `build.gradle` with content including `com.android.application`
- THEN the detector MUST scan `build.gradle`, `build.gradle.kts`, `settings.gradle`, and
  `settings.gradle.kts` for the specified patterns
- AND the technology MUST be detected if any Gradle file matches

#### Scenario: Empty detect block does not cause error

- GIVEN the catalog defines a technology entry with `id = "custom_tech"` and no `[detect]` block
- WHEN the catalog is loaded
- THEN the technology entry MUST be accepted as valid
- AND the technology MUST NOT be detected by the `CatalogDrivenDetector` (it has no detection rules)

#### Scenario: Invalid regex pattern is handled gracefully

- GIVEN the catalog defines a technology with `package_patterns = ["[invalid"]`
- WHEN the catalog is loaded
- THEN the system MUST log a warning about the invalid pattern
- AND the technology's detection rules MUST be skipped
- AND other technologies in the catalog MUST still load successfully

---

### Requirement: CatalogDrivenDetector

The system MUST provide a `CatalogDrivenDetector` struct that implements the `RepoDetector` trait
and evaluates catalog-defined detection rules against repository contents.

The `CatalogDrivenDetector` MUST accept a reference to a `ResolvedSkillCatalog` at construction
time.

For each technology in the catalog that has detection rules, the `CatalogDrivenDetector` MUST
evaluate detection rules in the following order:

1. `packages` — exact match against merged package.json dependency names
2. `package_patterns` — regex match against merged package.json dependency names
3. `config_files` — check file existence at the project root
4. `config_file_content` — read specified files and search for patterns

The detector MUST stop evaluating a technology's rules after the first successful match (
short-circuit).

Each successful detection MUST produce a `TechnologyDetection` containing the `TechnologyId`,
a `DetectionConfidence` value, and evidence describing the matched rule and artifact.

The confidence level MUST be `high` when the detection is based on exact package matches or
canonical config files, and `medium` for pattern-based matches or content scanning.

The `CatalogDrivenDetector` MUST reuse the existing `IGNORED_DIRS` list when walking the filesystem
to avoid scanning build artifacts, `node_modules`, `.git`, and similar directories.

The `CatalogDrivenDetector` MUST be deterministic for the same repository contents and catalog.

#### Scenario: Detect technology via exact package match

- GIVEN the catalog defines `vue` with `detect.packages = ["vue"]`
- AND the repository contains a `package.json` with `"vue": "^3.4.0"` in `dependencies`
- WHEN the `CatalogDrivenDetector` runs
- THEN the detection result MUST include a `TechnologyDetection` for `vue`
- AND confidence MUST be `high`
- AND evidence MUST reference the `vue` package in `package.json`

#### Scenario: Detect technology via config file existence

- GIVEN the catalog defines `tailwindcss` with
  `detect.config_files = ["tailwind.config.js", "tailwind.config.ts"]`
- AND the repository root contains `tailwind.config.js`
- WHEN the `CatalogDrivenDetector` runs
- THEN the detection result MUST include a `TechnologyDetection` for `tailwindcss`
- AND confidence MUST be `high`
- AND evidence MUST reference `tailwind.config.js`

#### Scenario: No detection when rules do not match

- GIVEN the catalog defines `svelte` with `detect.packages = ["svelte"]`
- AND the repository's `package.json` does not contain `"svelte"` in any dependency field
- AND no config files for svelte exist
- WHEN the `CatalogDrivenDetector` runs
- THEN the detection result MUST NOT include `svelte`

#### Scenario: Multiple technologies detected from same package.json

- GIVEN the catalog defines `react` with `detect.packages = ["react"]` and `vue` with
  `detect.packages = ["vue"]`
- AND the repository's `package.json` contains both `"react"` and `"vue"` in `dependencies`
- WHEN the `CatalogDrivenDetector` runs
- THEN the detection result MUST include both `react` and `vue`

#### Scenario: Short-circuit after first matching rule

- GIVEN the catalog defines a technology with both `packages = ["express"]` and
  `config_files = ["express.config.js"]`
- AND the repository's `package.json` contains `"express"` in `dependencies`
- WHEN the `CatalogDrivenDetector` evaluates that technology's rules
- THEN the technology MUST be detected via the `packages` rule
- AND the `config_files` rule MUST NOT be evaluated for that technology

---

### Requirement: Package.json Dependency Parsing

The `CatalogDrivenDetector` MUST parse `package.json` files to extract dependency names from the
`dependencies`, `devDependencies`, and `peerDependencies` fields.

The parser MUST handle well-formed JSON. If a `package.json` file is malformed or unreadable, the
system MUST skip it with a warning and MUST NOT fail the entire detection pass.

The parser MUST collect only package names (keys), not version specifiers (values).

The parser MUST merge dependency names from all parsed `package.json` files (root and workspace
packages) into a single deduplicated set before evaluating package-based detection rules.

#### Scenario: Parse dependencies from all three fields

- GIVEN a `package.json` containing:
  ```json
  {
    "dependencies": { "express": "^4.18.0" },
    "devDependencies": { "vitest": "^1.0.0" },
    "peerDependencies": { "react": "^18.0.0" }
  }
  ```
- WHEN the parser extracts dependency names
- THEN the result MUST include `"express"`, `"vitest"`, and `"react"`

#### Scenario: Malformed package.json is skipped

- GIVEN the repository root contains a `package.json` with invalid JSON
- WHEN the `CatalogDrivenDetector` attempts to parse it
- THEN the system MUST log a warning
- AND the detection pass MUST continue without that file's data
- AND no error MUST be returned to the caller

#### Scenario: Missing package.json is not an error

- GIVEN the repository root does not contain a `package.json`
- WHEN the `CatalogDrivenDetector` runs
- THEN the merged package set MUST be empty
- AND package-based detection rules MUST match nothing
- AND the detection pass MUST continue with other rule types (config_files, etc.)

---

### Requirement: Monorepo Workspace Resolution

The `CatalogDrivenDetector` MUST resolve monorepo workspaces to discover `package.json` files in
workspace packages, merging their dependency names with the root `package.json` dependencies.

The detector MUST support the following workspace declaration formats:

- **pnpm**: `pnpm-workspace.yaml` with a `packages` list of glob patterns
- **npm/yarn**: `package.json` `workspaces` field as an array of glob patterns
- **yarn (object form)**: `package.json` `workspaces.packages` field as an array of glob patterns

Workspace resolution MUST resolve glob patterns relative to the project root to find workspace
`package.json` files.

Workspace resolution MUST NOT recursively resolve nested workspace declarations (only the top-level
workspace config is honored).

If a workspace glob resolves to a directory, the system MUST look for `package.json` inside that
directory.

If a workspace `package.json` is malformed or unreadable, the system MUST skip it with a warning
and continue processing other workspaces.

#### Scenario: pnpm workspace resolution

- GIVEN the repository contains `pnpm-workspace.yaml` with:
  ```yaml
  packages:
    - "packages/*"
  ```
- AND `packages/ui/package.json` contains `"dependencies": { "vue": "^3.0.0" }`
- AND `packages/api/package.json` contains `"dependencies": { "express": "^4.0.0" }`
- AND the root `package.json` contains `"dependencies": { "typescript": "^5.0.0" }`
- WHEN the `CatalogDrivenDetector` resolves workspaces
- THEN the merged package set MUST include `"vue"`, `"express"`, and `"typescript"`

#### Scenario: npm/yarn workspaces field resolution

- GIVEN the repository's root `package.json` contains:
  ```json
  { "workspaces": ["packages/*", "apps/*"] }
  ```
- AND `packages/shared/package.json` contains `"dependencies": { "lodash": "^4.0.0" }`
- WHEN the `CatalogDrivenDetector` resolves workspaces
- THEN the merged package set MUST include `"lodash"` along with root dependencies

#### Scenario: yarn object-form workspaces

- GIVEN the repository's root `package.json` contains:
  ```json
  { "workspaces": { "packages": ["packages/*"] } }
  ```
- WHEN the `CatalogDrivenDetector` resolves workspaces
- THEN it MUST treat `"packages": ["packages/*"]` as the workspace glob list

#### Scenario: Malformed workspace package.json is skipped

- GIVEN a workspace glob resolves to `packages/broken/package.json` which contains invalid JSON
- WHEN the `CatalogDrivenDetector` resolves workspaces
- THEN the system MUST log a warning for that file
- AND other workspace `package.json` files MUST still be parsed
- AND the detection pass MUST continue

#### Scenario: No workspace configuration

- GIVEN the repository has no `pnpm-workspace.yaml` and the root `package.json` has no `workspaces`
  field
- WHEN the `CatalogDrivenDetector` resolves workspaces
- THEN only the root `package.json` dependencies MUST be used

---

### Requirement: Web Frontend Detection

The system MUST support a special `web_frontend` technology that is detected by scanning for
frontend-related file extensions in the repository.

The detector MUST scan for the following file extensions: `.html`, `.css`, `.scss`, `.sass`,
`.less`,
`.vue`, `.svelte`, `.jsx`, `.tsx`, `.astro`, `.mdx`.

The scan depth MUST be limited to a maximum of 3 directory levels from the project root.

The scan MUST respect the existing `IGNORED_DIRS` list.

If at least one matching file is found, the `web_frontend` technology MUST be detected with
confidence `medium`.

The evidence MUST list the matched file extensions found (not every individual file).

The `web_frontend` technology MUST be associated with bonus skills for frontend design,
accessibility, and SEO in the catalog.

#### Scenario: Frontend files trigger web_frontend detection

- GIVEN a repository containing `src/App.tsx` and `src/styles/main.scss`
- WHEN the `CatalogDrivenDetector` runs
- THEN the detection result MUST include `web_frontend`
- AND confidence MUST be `medium`
- AND evidence MUST mention the `.tsx` and `.scss` extensions

#### Scenario: No frontend files means no web_frontend detection

- GIVEN a repository containing only `Cargo.toml`, `src/main.rs`, and `Makefile`
- WHEN the `CatalogDrivenDetector` runs
- THEN the detection result MUST NOT include `web_frontend`

#### Scenario: Scan depth limit is respected

- GIVEN a repository containing `deeply/nested/level4/component.vue` (4 levels deep)
- AND no frontend files exist within the first 3 directory levels
- WHEN the `CatalogDrivenDetector` runs
- THEN `web_frontend` MUST NOT be detected

#### Scenario: web_frontend maps to bonus skills

- GIVEN the catalog defines `web_frontend` with skills
  `["frontend-design", "accessibility", "seo"]`
- AND `web_frontend` is detected in a repository
- WHEN the recommendation engine evaluates detections
- THEN the recommendations MUST include `frontend-design`, `accessibility`, and `seo`

---

### Requirement: Combo Evaluation

The system MUST evaluate combo entries with `enabled = true` after all individual technology
detections have been collected.

A combo MUST be considered matching when ALL technologies listed in its `requires` field are present
in the detection results.

When a combo matches, its listed skills MUST be added to the skill recommendations with a reason
referencing the combo name and the contributing technologies.

Combos with `enabled = false` or without an explicit `enabled` field MUST NOT be evaluated.

Combo skill recommendations MUST participate in the same deduplication logic as technology skill
recommendations — if a combo recommends a skill already recommended by a technology, the reasons
and matched technologies MUST be merged into a single recommendation entry.

#### Scenario: Enabled combo matches and adds skills

- GIVEN the catalog defines a combo:
  ```toml
  [[combos]]
  id = "fullstack-node"
  name = "Full-Stack Node"
  requires = ["node_typescript", "web_frontend"]
  skills = ["performance"]
  enabled = true
  ```
- AND the detection results include both `node_typescript` and `web_frontend`
- WHEN combo evaluation runs
- THEN the recommendations MUST include `performance`
- AND the recommendation reason MUST reference the combo name "Full-Stack Node"

#### Scenario: Disabled combo is not evaluated

- GIVEN the catalog defines a combo with `enabled = false`
- AND all its required technologies are detected
- WHEN combo evaluation runs
- THEN the combo's skills MUST NOT be added to recommendations

#### Scenario: Combo with missing required technology does not match

- GIVEN the catalog defines a combo requiring `["docker", "kubernetes"]`
- AND only `docker` is detected (no `kubernetes`)
- WHEN combo evaluation runs
- THEN the combo MUST NOT match
- AND its skills MUST NOT be added to recommendations

#### Scenario: Combo skill deduplication with technology skill

- GIVEN the catalog defines technology `astro` with skills `["frontend-design"]`
- AND a combo with skills `["frontend-design"]` also matches
- WHEN recommendations are generated
- THEN `frontend-design` MUST appear only once in the recommendations
- AND its `matched_technologies` and `reasons` MUST include contributions from both the technology
  and the combo

---

### Requirement: Expanded Catalog Content

The embedded `catalog.v1.toml` MUST be expanded to include the full autoskills ecosystem data:
approximately 46 technologies, approximately 150 skills, and approximately 11 combos.

Each skill entry MUST include `provider_skill_id`, `local_skill_id`, `title`, and `summary`.

Skills from the autoskills ecosystem MUST use the `owner/repo/skill-name` format for
`provider_skill_id` (e.g., `"acosta/agentsync-skills/vue"`).

Each technology entry MUST include `id`, `name`, `skills`, and MAY include `detect`,
`min_confidence`, and `reason_template`.

Each combo entry MUST include `id`, `name`, `requires`, `skills`, and `enabled`.

The catalog MUST remain valid TOML and MUST pass schema validation on load.

The catalog MUST be embedded at compile time via `include_str!` with no runtime network access.

#### Scenario: Catalog loads successfully with expanded content

- GIVEN the embedded `catalog.v1.toml` contains ~46 technologies, ~150 skills, and ~11 combos
- WHEN the system loads the embedded catalog
- THEN the catalog MUST parse without errors
- AND all technology entries MUST have valid `id`, `name`, and `skills` fields
- AND all combo entries MUST have valid `id`, `name`, `requires`, and `skills` fields

#### Scenario: Skills use owner/repo/skill-name format

- GIVEN the catalog defines a skill with `provider_skill_id = "acosta/agentsync-skills/vue"`
- WHEN the catalog is loaded
- THEN the skill definition MUST be accepted as valid
- AND the `provider_skill_id` MUST be preserved for use in provider resolution

---

### Requirement: Provider Resolve for owner/repo/skill-name Format

`SkillsShProvider.resolve()` MUST handle the `owner/repo/skill-name` format used by autoskills
skill IDs.

When the skill ID contains two or more `/` separators (indicating `owner/repo/skill-name` format),
the resolver MUST construct the GitHub download URL deterministically from the first two path
segments (`owner/repo`) and the final path segment (`skill-name`) without requiring a search API
round-trip.

The resolver MUST preserve backward compatibility with simple skill IDs (e.g., `"docker-expert"`)
that contain no `/` separators.

#### Scenario: Resolve owner/repo/skill-name format

- GIVEN a skill ID `"acosta/agentsync-skills/vue"`
- WHEN `SkillsShProvider.resolve()` is called with that ID
- THEN the download URL MUST be constructed from `"acosta/agentsync-skills"`
- AND the resolver MUST preserve the `"vue"` path segment when deriving the archive subpath

#### Scenario: Simple skill ID still resolves

- GIVEN a skill ID `"docker-expert"`
- WHEN `SkillsShProvider.resolve()` is called with that ID
- THEN the resolver MUST behave identically to the current implementation
- AND backward compatibility MUST be preserved

---

### Requirement: Backward Compatibility

The system MUST maintain full backward compatibility for the 7 existing technologies (`rust`,
`node_typescript`, `astro`, `github_actions`, `docker`, `make`, `python`).

The migration to catalog-driven detection MUST preserve the observable behavior of the original 7
technologies even though detection is now performed by `CatalogDrivenDetector` alone.

`FileSystemRepoDetector` is not required to remain in the runtime architecture once the original 7
technology patterns have been fully migrated into catalog `detect` blocks.

The JSON output contract for `skill suggest` MUST remain unchanged: `detections`,
`recommendations`, and `summary` with the same field shapes.

The human-readable CLI output format for `skill suggest` MUST remain materially equivalent.

The `skill suggest --install` flow MUST continue to work with both existing and newly discovered
skills.

No breaking changes MUST be introduced to any CLI subcommand interface.

#### Scenario: Existing 7 technologies produce identical results

- GIVEN a repository containing `Cargo.toml`, `package.json`, `Dockerfile`, `Makefile`, and
  `.github/workflows/ci.yml`
- AND no new catalog-driven technologies match additional rules
- WHEN the user runs `skill suggest --json`
- THEN the JSON output MUST be identical to the output produced before this change
  (same detections, same recommendations, same summary counts)

#### Scenario: New technologies extend but do not break existing output

- GIVEN a repository containing `Cargo.toml` and `package.json` with `"vue": "^3.0.0"`
- WHEN the user runs `skill suggest --json`
- THEN the JSON output MUST include the existing `rust` and `node_typescript` detections
- AND the JSON output MUST additionally include the `vue` detection
- AND the `summary.detected_count` MUST reflect the total including new detections

#### Scenario: Catalog-driven detection preserves legacy rust result

- GIVEN a repository containing `Cargo.toml`
- AND the catalog defines `rust` with `config_files = ["Cargo.toml"]`
- WHEN detection runs through `CatalogDrivenDetector`
- THEN the final result MUST contain exactly one `rust` detection
- AND the confidence MUST be `high`

#### Scenario: skill suggest --install works with new skills

- GIVEN a repository where `vue` is detected and `vue` skill is recommended but not installed
- WHEN the user runs `skill suggest --install`
- THEN the `vue` skill MUST appear in the interactive selection
- AND selecting it MUST trigger installation through the existing `SkillsShProvider` flow

---

### Requirement: Detection Performance

The `CatalogDrivenDetector` MUST complete detection within 2 seconds for typical repositories
(< 10,000 files, < 50 workspace packages).

The detector MUST compile all regex patterns once during catalog loading, not per evaluation.

The detector MUST reuse the `IGNORED_DIRS` list to skip irrelevant directories.

The detector MUST limit web frontend file extension scanning to a maximum depth of 3 directory
levels.

The detector SHOULD parse `package.json` files only once and cache the merged dependency set for
reuse across all technology rule evaluations.

#### Scenario: Detection completes within time budget

- GIVEN a typical monorepo with 5,000 files and 10 workspace packages
- WHEN the user runs `skill suggest`
- THEN the detection phase MUST complete within 2 seconds

#### Scenario: Regex patterns are compiled once

- GIVEN the catalog defines 20 technologies with `package_patterns` containing regex
- WHEN the catalog is loaded
- THEN all regex patterns MUST be compiled during loading
- AND subsequent detection evaluations MUST NOT recompile patterns
