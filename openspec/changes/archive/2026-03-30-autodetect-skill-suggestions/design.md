# Design: Autodetect Skill Suggestions

## Technical Approach

This change adds a repo-aware recommendation pipeline under the existing `skill` command surface
without changing how skills are actually installed. The implementation is split into three layers
inside `src/skills/`: (1) repository technology detection, (2) catalog-backed recommendation policy,
and (3) install orchestration that reuses the existing `install`, `update`, and registry modules.

Phase 1 adds a read-only `agentsync skill suggest` flow that scans the project root, produces
normalized detections, resolves opinionated recommendations from an embedded catalog, annotates them
with installed-state from `.agents/skills/registry.json`, and prints either human-readable or JSON
output. Phase 2 extends the same pipeline with guided installation and `--install` /
`--install --all` paths, but delegates each selected install back to the existing install primitives
rather than creating a second installer.

This maps directly to the proposal: detection is local and deterministic, recommendation metadata is
abstracted behind a catalog boundary, registry remains the source of truth for installed state only,
and future provider-backed metadata can be introduced by swapping catalog sources instead of
redesigning CLI contracts.

## Architecture Decisions

### Decision: Separate detection, recommendation, and installation into distinct skills-domain modules

**Choice**: Create dedicated modules for repository detection and recommendation orchestration under
`src/skills/`, with `src/commands/skill.rs` acting only as CLI argument parsing and output
orchestration.

**Alternatives considered**: Extend `src/init.rs` scanning directly; embed all logic in
`src/commands/skill.rs`.

**Rationale**: `init.rs` already contains migration-specific scanning and interactive UX, but the
new feature belongs to the skill lifecycle, not adoption. A dedicated skills-domain pipeline keeps
`init` focused, makes recommendation logic reusable across read-only and install flows, and matches
the existing pattern where commands delegate to skill-domain modules.

### Decision: Introduce a catalog abstraction with embedded v1 data and optional provider-backed sources later

**Choice**: Add a `SkillCatalog` abstraction plus an embedded catalog implementation that ships
deterministic metadata in the binary. Provider-backed catalog loading is modeled as a future
additional implementation, not a v1 requirement.

**Alternatives considered**: Hardcode recommendation rules directly in CLI code; immediately
redesign `Provider` into a full remote catalog client.

**Rationale**: The existing `Provider` trait is currently optimized for `resolve()` and is too thin
for a forced catalog-first design. A separate catalog boundary preserves offline/deterministic
behavior now while leaving a clean future path for network-backed metadata.

### Decision: Keep installed-state awareness in the existing registry and do not expand registry schema into a catalog cache

**Choice**: Read `.agents/skills/registry.json` to annotate suggestions as installed/not-installed
and to surface installed version metadata when present, but do not persist recommendation metadata
to the registry in v1.

**Alternatives considered**: Extend the registry to cache catalog entries, rankings, or detection
snapshots.

**Rationale**: The current registry is install-state-oriented and already powers
install/update/uninstall flows. Reusing it for installed awareness avoids schema churn and keeps
recommendation metadata ephemeral, which is better aligned with a future provider-backed catalog.

### Decision: Use

`skill suggest` as the repo-aware entry point and layer phase-2 installs onto that flow

**Choice**: Add `agentsync skill suggest` in phase 1, then extend it in phase 2 with
install-oriented flags (`--install` for guided selection and `--install --all` for the explicit
non-interactive install-all path) while internally invoking the same install logic used by
`skill install`.

**Alternatives considered**: Add repo-aware flags to `skill install`; create a standalone top-level
command.

**Rationale**: `suggest` is the discovery-oriented entry point and keeps recommendation UX grouped
under `skill`. This preserves the simple single-skill semantics of `skill install`, while still
allowing guided install to reuse the exact same lifecycle primitives internally.

### Decision: Use conservative, evidence-backed heuristics with confidence levels to mitigate monorepo false positives

**Choice**: Detection returns evidence paths and confidence levels. Recommendation rules only fire
for strong signals by default, with special-case handling for nested first-party app markers (for
example `astro.config.*` under a non-ignored subtree).

**Alternatives considered**: Any-match equals recommendation; full project-graph analysis.

**Rationale**: File-marker detection is intentionally shallow in v1, so confidence gating is the
simplest way to avoid recommending Docker or framework skills because of incidental test fixtures,
vendored code, or generated artifacts.

## Data Flow

### Phase 1: Read-only suggestion

```text
CLI (`skill suggest`)
   │
   ▼
SuggestionService
   ├── RepoDetector.scan(project_root)
   │      └── returns TechnologyDetections + evidence
   ├── RegistryInstalledState.load(.agents/skills/registry.json)
   │      └── returns installed skill map (or empty if absent)
   ├── SkillCatalog.recommend(detections)
   │      └── returns candidate suggestions + matched rules
   └── annotate/install-state + render human/JSON output
```

### Phase 2: Guided install

```text
TTY user ──► `skill suggest --install`
                │
                ▼
         SuggestionService.plan()
                │
                ▼
      dialoguer MultiSelect of non-installed suggestions
                │
                ▼
     install_selected_suggestions(selected_ids)
                │
                ├── resolve source/provider per suggestion
                └── reuse existing install::blocking_fetch_and_install_skill
```

### Sequence diagram

```text
User
 │  skill suggest --json
 ▼
Skill CLI
 │
 │ build SuggestRequest
 ▼
SuggestionService
 ├──► RepoDetector
 │      returns detections[evidence, confidence]
 ├──► EmbeddedCatalog (v1)
 │      returns suggestions[rules matched]
 ├──► RegistryReader
 │      returns installed skills
 └──► OutputMapper
        returns SuggestResponse
 ▼
stdout (human or JSON)
```

## File Changes

| File                                           | Action | Description                                                                                                                          |
|------------------------------------------------|--------|--------------------------------------------------------------------------------------------------------------------------------------|
| `src/skills/mod.rs`                            | Modify | Export new detection/catalog/recommendation modules.                                                                                 |
| `src/skills/detect.rs`                         | Create | Repository technology detector, evidence model, confidence scoring, ignored-directory pruning.                                       |
| `src/skills/catalog.rs`                        | Create | Catalog trait, embedded catalog implementation, rule metadata types, future provider-backed adapter seam.                            |
| `src/skills/suggest.rs`                        | Create | Suggestion orchestration service, installed-state annotation, JSON/view-model mapping, phase-2 install planning helpers.             |
| `src/skills/provider.rs`                       | Modify | Add optional metadata-provider compatibility seam or adapter types for future catalog-backed providers without changing v1 behavior. |
| `src/skills/registry.rs`                       | Modify | Add small read helpers for installed-state lookup/annotation while keeping registry schema unchanged.                                |
| `src/commands/skill.rs`                        | Modify | Add `Suggest` subcommand, CLI flags, human/JSON rendering, and phase-2 install entry points.                                         |
| `src/main.rs`                                  | Modify | Wire any new `skill` subcommand metadata if needed by clap help text.                                                                |
| `tests/unit/suggest_detector.rs`               | Create | Unit coverage for marker detection, confidence scoring, and ignored-path behavior.                                                   |
| `tests/unit/suggest_catalog.rs`                | Create | Unit coverage for rule matching, deduplication, and installed-state annotation.                                                      |
| `tests/contracts/test_skill_suggest_output.rs` | Create | JSON contract tests for read-only suggestion output and error output.                                                                |
| `tests/integration/skill_suggest.rs`           | Create | CLI integration tests for suggest behavior and phase-2 install orchestration.                                                        |

## Interfaces / Contracts

### Core domain types

```rust
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum TechnologyId {
    Rust,
    #[serde(rename = "node_typescript")]
    NodeTypeScript,
    Astro,
    #[serde(rename = "github_actions")]
    GitHubActions,
    Docker,
    Make,
    Python,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionConfidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DetectionEvidence {
    pub marker: String,
    pub path: PathBuf,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TechnologyDetection {
    pub technology: TechnologyId,
    pub confidence: DetectionConfidence,
    pub root_relative_paths: Vec<PathBuf>,
    pub evidence: Vec<DetectionEvidence>,
}

#[derive(Debug, Clone)]
pub struct CatalogRule {
    pub id: String,
    pub any_of: Vec<TechnologyId>,
    pub all_of: Vec<TechnologyId>,
    pub min_confidence: DetectionConfidence,
    pub skill_id: String,
    pub reason_template: String,
    pub phase: SuggestionPhase,
}

#[derive(Debug, Clone)]
pub struct CatalogSkillMetadata {
    pub skill_id: String,
    pub title: String,
    pub summary: String,
    pub source: CatalogSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstalledState {
    NotInstalled,
    Installed,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillSuggestion {
    pub skill_id: String,
    pub title: String,
    pub summary: String,
    pub reason: String,
    pub matched_technologies: Vec<TechnologyId>,
    pub installed_state: InstalledState,
    pub installed_version: Option<String>,
    pub catalog_source: String,
}
```

### Detector boundary

```rust
pub trait RepoDetector {
    fn detect(&self, project_root: &std::path::Path) -> anyhow::Result<Vec<TechnologyDetection>>;
}
```

Detector rules in v1:

- prune generated/vendor directories early: `.git`, `.agents`, `node_modules`, `target`, `dist`,
  `build`, `.astro`, `.next`, `.turbo`, `.pnpm-store`
- root-level markers produce `High` confidence
- nested first-party markers produce `Medium` confidence when found outside ignored paths
- incidental/test-only markers produce `Low` confidence and do not trigger default recommendations

Examples:

- `Cargo.toml` at repo root → `rust/high`
- `.github/workflows/*.yml|yaml` at repo root → `github_actions/high`
- `Dockerfile` or `docker-compose*.yml` at repo root → `docker/high`
- only `tests/e2e/Dockerfile.e2e` → `docker/low`
- `website/docs/astro.config.mjs` outside ignored dirs → `astro/medium`

### Catalog boundary

```rust
pub trait SkillCatalog {
    fn metadata_version(&self) -> &str;

    fn list_rules(&self) -> &[CatalogRule];

    fn get_skill(&self, skill_id: &str) -> Option<&CatalogSkillMetadata>;
}
```

V1 embedded catalog contents are compiled into the binary and map technologies to opinionated skills
such as:

- `rust` → `rust-async-patterns`
- `node_typescript` / `python` → `best-practices`
- `astro` → `frontend-design`, `accessibility`, `performance`, `core-web-vitals`, `seo`
- `github_actions` → `github-actions`, `pinned-tag`
- `docker` → `docker-expert`
- `make` → `makefile`

The rule engine deduplicates by `skill_id`, merges reasons/technologies, and preserves deterministic
ordering via `BTreeMap`/sorted rule iteration.

### Suggest CLI contract

Phase 1 command surface:

```text
agentsync skill suggest [--json]
```

Phase 2 extensions:

```text
agentsync skill suggest --install
agentsync skill suggest --install --all
```

Rules:

- plain `skill suggest` stays read-only
- `--install` requires a TTY when used without `--all` and only presents not-yet-installed
  suggestions
- `--install --all` is the explicit non-interactive install-all path for CI/scripts
- all install paths reuse the existing install pipeline for each selected skill id

### JSON output contract

Phase 1 read-only output:

```json
{
  "detections": [
    {
      "technology": "rust",
      "confidence": "high",
      "evidence": ["Cargo.toml"]
    }
  ],
  "recommendations": [
    {
      "skill_id": "rust-async-patterns",
      "matched_technologies": ["rust"],
      "reasons": ["Recommended because Rust was detected from Cargo.toml."],
      "installed": false
    }
  ],
  "summary": {
    "detected_count": 1,
    "recommended_count": 1,
    "installable_count": 1
  }
}
```

Phase 2 install output extends the same response with install metadata:

```json
{
  "detections": [
    {
      "technology": "github_actions",
      "confidence": "high",
      "evidence": [".github/workflows/release.yml"]
    }
  ],
  "recommendations": [
    {
      "skill_id": "github-actions",
      "matched_technologies": ["github_actions"],
      "reasons": [
        "Recommended because GitHub Actions workflows were detected from .github/workflows/release.yml."
      ],
      "installed": true
    },
    {
      "skill_id": "pinned-tag",
      "matched_technologies": ["github_actions"],
      "reasons": [
        "Recommended because GitHub Actions workflows were detected from .github/workflows/release.yml."
      ],
      "installed": true
    }
  ],
  "summary": {
    "detected_count": 1,
    "recommended_count": 2,
    "installable_count": 0
  },
  "mode": "install_all",
  "selected_skill_ids": ["github-actions", "pinned-tag"],
  "results": [
    { "skill_id": "github-actions", "status": "installed" },
    { "skill_id": "pinned-tag", "status": "already_installed" }
  ]
}
```

If a selection is invalid (for example a requested skill is not part of the current suggestion set),
the command returns a non-zero exit with the existing error shape:

```json
{
  "error": "requested skill is not part of the current suggestion set",
  "code": "invalid_suggestion_selection",
  "remediation": "Run 'agentsync skill suggest --json' to inspect available suggested skill ids."
}
```

## Testing Strategy

| Layer       | What to Test                                                                           | Approach                                                                                                                                                                   |
|-------------|----------------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| Unit        | Detection markers, ignored directories, confidence scoring, false-positive suppression | TempDir fixtures with root vs nested markers and vendor/build-path pruning assertions.                                                                                     |
| Unit        | Catalog rule matching, deduplication, ordering, installed-state annotation             | Pure Rust tests over embedded rules plus fake registry contents.                                                                                                           |
| Unit        | Provider-backed seam fallback                                                          | Tests that embedded catalog is used when no provider metadata source is configured/available.                                                                              |
| Integration | `skill suggest` human output                                                           | Spawn CLI in fixture repos and assert suggested sections, installed annotations, and no filesystem mutation in read-only mode.                                             |
| Contract    | `skill suggest --json` success/error shape                                             | Parse stdout as JSON and assert required keys, stable enums, and error codes/remediation fields.                                                                           |
| Integration | Installed-skill awareness                                                              | Pre-populate `.agents/skills/registry.json` and assert already-installed suggestions are annotated and skipped by `--install` / `--install --all`.                         |
| Integration | Phase-2 non-interactive installs                                                       | Run `skill suggest --install --all` against local fixture skills and assert delegation through existing install behavior.                                                  |
| Integration | TTY interactive flow                                                                   | Add targeted tests around suggestion selection orchestration where feasible; keep prompt internals thin and most logic covered via service-level tests.                    |
| Regression  | Monorepo mitigation                                                                    | Fixture repos with nested docs app, test-only Dockerfile, and generated directories to verify which technologies become recommendations vs low-confidence-only detections. |

## Migration / Rollout

No migration required.

Rollout plan:

1. Phase 1 ships `skill suggest` as read-only with embedded catalog metadata.
2. Phase 2 adds guided install flags on the same command and reuses existing install primitives.
3. Provider-backed catalog support can be added later behind `SkillCatalog` without changing output
   schemas or registry storage.

## Open Questions

- [ ] Confirm whether a single nested `astro.config.*` in a docs/app subtree should always emit
  recommendations or only when paired with root/node workspace evidence.
- [x] Confirmed phase-2 flag names are `--install` and `--install --all`.
