## Exploration: autodetect skill suggestions

### Current State

AgentSync already has solid primitives for skill lifecycle management, but not for recommendation
discovery. `agentsync skill install|update|uninstall` is implemented in `src/commands/skill.rs` and
delegates to `src/skills/{install,update,uninstall}.rs`. Installed skills live under
`.agents/skills/{skill_id}` and registry metadata is persisted in `.agents/skills/registry.json` via
`src/skills/registry.rs`. The current registry schema is minimal (`schemaVersion`, `last_updated`,
per-skill name/version/description/provider/source/installedAt/files/manifestHash`) and is focused
on installed-state tracking, not catalog metadata.

Provider support is also minimal today. `src/skills/provider.rs` exposes a `Provider` trait, but
`commands/skill.rs` only uses `SkillsShProvider.resolve()` to map a skill id to a download URL. The
provider `manifest()` API is currently unused, so there is not yet an active abstraction for
fetching a recommendation catalog or typed metadata.

Repo scanning already exists, but only for agent-adoption workflows. `src/init.rs` contains
`scan_agent_files()` plus a large `AgentFileType` enum for discovering existing agent instructions,
skills, commands, and MCP config files. `init_wizard()` uses `dialoguer` interactive prompts (
`Confirm`, `MultiSelect`, `Select`) to migrate detected artifacts into `.agents/`. This is the
closest existing pattern for repository analysis + interactive selection. Adoption tests in
`tests/test_agent_adoption.rs` confirm this init → migrate → apply flow.

CLI machine-readable conventions are lightweight and command-specific. `status --json` prints pretty
JSON arrays and exits non-zero when drift exists. `skill install|update|uninstall --json` print
compact JSON objects with success payloads or `{ error, code, remediation }` on failure. There is no
shared output envelope type yet, and `skill list` is still unimplemented.

For v1 repository technology detection, the repo already demonstrates the file markers likely
needed: `Cargo.toml` (Rust), `package.json` (Node/TypeScript ecosystem),
`website/docs/astro.config.mjs` (Astro), `.github/workflows/*.yml` (GitHub Actions), `Dockerfile` /
`tests/e2e/Dockerfile.e2e` (Docker), `Makefile` (Make). There is no existing generic repository-tech
detector in `src/` today.

### Affected Areas

- `src/commands/skill.rs` — best CLI insertion point for new recommendation-oriented subcommands and
  JSON output conventions.
- `src/skills/provider.rs` — best abstraction point for evolving from simple resolver to
  provider-backed recommendation/catalog metadata.
- `src/skills/registry.rs` — current installed-skill state store; could remain install-state-only or
  grow to cache recommendation/catalog metadata.
- `src/skills/install.rs` / `src/skills/update.rs` / `src/skills/uninstall.rs` — existing lifecycle
  primitives that a recommendation flow should reuse rather than bypass.
- `src/init.rs` — existing repository scanning and interactive selection patterns; strongest
  precedent for “analyze repo, then let user choose”.
- `src/main.rs` — Clap subcommand surface where a new `skill suggest`/`recommend` command would be
  registered.
- `tests/contracts/test_install_output.rs` and `tests/test_update_output.rs` — examples of current
  JSON command contracts that recommendation flows should mirror.
- `tests/test_agent_adoption.rs` — existing coverage for repository scanning/adoption patterns,
  useful precedent for recommendation detection tests.
- `src/config.rs` — defines config/source-dir conventions and may matter if detection should respect
  discovered project roots or future config knobs.

### Approaches

1. **Embedded detector + embedded recommendation catalog** — Add a local Rust detector for repo
   markers and a built-in mapping from detected technologies to recommended skills.
    - Pros: Fastest path to ship, no network dependency, deterministic tests, fits current
      architecture, cleanly reuses `init.rs` scanning style and existing install primitives.
    - Cons: Catalog updates require binary releases, broader ecosystem coverage will grow code
      size/churn, metadata model may need later reshaping.
    - Effort: Medium

2. **Provider/catalog-first recommendations** — Introduce a metadata provider now, fetch a
   recommendation catalog from a remote/provider source, and drive suggestions from that catalog
   immediately.
    - Pros: Best long-term fit for “provider-backed metadata”, easier to expand coverage without
      binary releases, cleaner separation between detection and catalog policy.
    - Cons: Higher upfront design cost, offline/error-handling complexity, no strong existing
      catalog client abstraction yet, harder to stabilize v1 quickly.
    - Effort: High

### Recommendation

Use a clean two-phase design and implement phase 1 with explicit interfaces that phase 2 can swap
behind.

Best-fit implementation shape:

- Add a new recommendation-oriented CLI path under `src/commands/skill.rs` and `src/main.rs` rather
  than overloading `init`; this keeps skill discovery/install concerns together.
- Extract repository technology detection into a dedicated module (new module is preferable to
  extending `init.rs` directly), but borrow `init.rs`'s file-system scanning style and `dialoguer`
  interaction patterns.
- Keep recommendation logic separate from installation logic. Detection should output normalized
  “detected technologies”; recommendation logic should map those technologies to “recommended
  skills”; install should continue to call existing lifecycle primitives.
- Define a catalog interface early (even if backed by embedded data in v1) so phase 2 can replace
  the embedded catalog with provider-backed metadata without redesigning the command UX.

Suggested v1 detection scope from current repo conventions and PRD:

- Rust: `Cargo.toml`
- TypeScript/Node: `package.json`, optionally strengthened later with `tsconfig.json`
- Astro: `astro.config.*`
- GitHub Actions: `.github/workflows/*.yml` or `*.yaml`
- Docker: `Dockerfile*`, `docker-compose*.yml`, `compose.yml`
- Make: `Makefile`
- Python: `pyproject.toml`, `requirements*.txt`, optionally `uv.lock`/`poetry.lock` later

Recommended phase split:

- **Phase 1:** local detection + embedded recommendation metadata + CLI/JSON/interactive UX + reuse
  existing install flow.
- **Phase 2:** replace or augment embedded metadata with provider/catalog-backed metadata, optional
  caching, richer ranking, and broader ecosystem coverage.

This split is clean because detection heuristics and install execution are local product behavior,
while recommendation metadata freshness/ranking is the part most likely to evolve externally.

### Risks

- Current provider abstraction is too thin for catalogs; forcing provider-backed metadata
  immediately will likely cause a premature redesign.
- Current registry is install-state-oriented; mixing installed state and remote catalog cache in one
  schema may create migration complexity.
- CLI JSON output is not standardized across commands today, so introducing recommendation JSON
  without a typed envelope could add more drift.
- `init.rs` already carries substantial adoption logic; placing generic repo-tech detection there
  would increase coupling unless extracted into a dedicated module.
- Detection by file markers alone can produce false positives in monorepos or repos with incidental
  tooling files.
- Selective vs “install all” UX should be designed carefully for both TTY and non-TTY modes; current
  interactive patterns exist only in init/adoption.

### Ready for Proposal

Yes — the codebase has enough precedent to propose a phased change. The proposal should explicitly
separate detector interfaces, recommendation catalog interfaces, CLI UX/JSON contract, and the
migration plan from embedded metadata to provider-backed metadata.
