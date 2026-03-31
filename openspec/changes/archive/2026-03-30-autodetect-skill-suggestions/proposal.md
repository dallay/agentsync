# Proposal: Autodetect Skill Suggestions

## Intent

AgentSync can install, update, and uninstall skills, but it cannot yet help users discover which skills fit a repository. Today users must already know the right skill IDs, even though the product already has strong precedents for repository scanning (`src/init.rs`), interactive selection (`dialoguer` in the init wizard), and installed-skill lifecycle management (`src/commands/skill.rs`, `src/skills/install.rs`, `src/skills/registry.rs`).

This change adds a technology-aware recommendation flow that detects common repository markers and suggests opinionated skills before installation. The goal is to make skill adoption faster and more guided while reusing existing install and registry flows instead of creating a parallel system.

## Desired Outcome

- Users can run a read-only suggestion command to see recommended skills for the current repository.
- Suggestions can be consumed in both human-friendly and JSON formats.
- A follow-on guided install flow can let users choose suggested skills or install all suggested skills without redefining install behavior.
- The v1 design stays local and deterministic, while explicitly preserving a path toward provider-backed recommendation metadata later.

## Scope

### In Scope
- Add a new recommendation-oriented skill command surface under the existing `skill` CLI.
- Detect repository technologies from local file markers for the initial v1 ecosystem: Rust, TypeScript/Node, Astro, GitHub Actions, Docker, Make, and Python.
- Introduce a normalized recommendation model that separates:
  - repository technology detection,
  - recommendation/catalog policy,
  - skill installation execution.
- Define phase 1 as a read-only `suggest` experience with human output and `--json` support.
- Define phase 2 as a guided installation flow that reuses existing `skill install` and registry behavior, including selective install and `install-all` support.
- Establish an interface boundary so embedded metadata used in v1 can later be replaced or augmented by provider-backed catalog metadata.

### Out of Scope / Non-Goals
- Building a full remote catalog client or mandatory network-backed recommendation system in v1.
- Expanding the installed-skill registry into a broad remote catalog cache during this change.
- Solving monorepo-wide project graph detection, ranking sophistication, or deep language framework inference beyond agreed file-marker heuristics.
- Replacing `init` or moving recommendation UX into the adoption wizard.
- Implementing a universal JSON envelope across every existing CLI command as part of this proposal.

## Phased Rollout

### Phase 1: Read-only suggest
- Ship local repository technology detection and embedded recommendation metadata.
- Add a `skill suggest`-style command that reports detected technologies and recommended skills.
- Support both terminal-friendly output and machine-readable JSON.
- Make no filesystem changes unless a later install command is explicitly invoked.

### Phase 2: Guided install
- Add interactive guided installation for suggested skills using existing `dialoguer` patterns where TTY is available.
- Support non-interactive selection paths, including an `--install-all` option for all recommended skills.
- Reuse existing install, update, uninstall, and registry flows rather than duplicating lifecycle logic.
- Keep the catalog boundary intact so provider-backed metadata can be introduced without redesigning the CLI contract.

## Approach

Implement this as a skill-domain feature, not an init feature. The existing `src/commands/skill.rs` surface is the correct entry point because recommendation and installation belong to the same user workflow, while `src/init.rs` remains the precedent for scanning style and interactive UX.

The implementation should introduce a dedicated repository-technology detection module instead of extending `init.rs` directly. Detection should emit normalized technology identifiers derived from local markers already visible in this repository, including `Cargo.toml`, `package.json`, `astro.config.*`, `.github/workflows/*`, `Dockerfile*`, `docker-compose*.yml`, `compose.yml`, `Makefile`, and common Python manifests.

Recommendation policy should live behind a catalog/recommendation interface. In v1, that interface is backed by embedded metadata compiled into the binary for deterministic offline behavior and straightforward tests. Installation must remain delegated to existing lifecycle primitives in `src/skills/install.rs` and related registry handling in `src/skills/registry.rs`.

## Repo Reality Anchors

- `src/commands/skill.rs` already owns install/update/uninstall and is the natural home for recommendation subcommands.
- `src/skills/provider.rs` already exposes a provider trait, but its metadata path is too thin today for a full catalog-first design; that makes an interface-first, embedded-v1 approach the pragmatic choice.
- `src/init.rs` already demonstrates repository scanning and guided terminal selection patterns worth reusing.
- `tests/contracts/test_install_output.rs` and `tests/test_update_output.rs` show existing JSON contract expectations that the suggestion flow should align with where practical.
- The current repo itself contains the exact marker files needed to validate the initial detection scope.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `src/main.rs` | Modified | Register new skill recommendation command surface and flags |
| `src/commands/skill.rs` | Modified | Add suggest/guided install entry points and output handling |
| `src/skills/` | New/Modified | Add detector/catalog domain modules and reuse existing install primitives |
| `src/skills/provider.rs` | Modified | Prepare for future provider-backed recommendation metadata without requiring it in v1 |
| `src/skills/registry.rs` | Reviewed/Maybe Modified | Keep install-state responsibilities clear and avoid premature catalog-cache coupling |
| `src/init.rs` | Referenced | Reuse scanning and interactive UX patterns without relocating recommendation logic there |
| `tests/` | Modified | Add detection, JSON contract, and guided install coverage |
| `openspec/changes/2026-03-30-autodetect-skill-suggestions/` | Modified | Add proposal now; specs/design/tasks to follow |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| File-marker detection produces false positives, especially in monorepos | Medium | Keep v1 heuristics explicit and conservative; model detection output separately from install decisions |
| Recommendation metadata shape may need to evolve for provider-backed catalogs | Medium | Introduce a catalog interface now and keep embedded metadata behind it |
| Guided install UX diverges between TTY and non-TTY contexts | Medium | Split read-only suggest from install flow and specify both interactive and non-interactive paths |
| Registry responsibilities become blurred by recommendation metadata | Medium | Keep registry focused on installed state in v1 and defer cache decisions |
| JSON output adds another ad hoc contract | Low | Define an explicit suggest contract in specs and align it with existing command patterns |

## Rollback Plan

If the feature causes confusing recommendations or unstable CLI behavior, remove the new recommendation subcommands and any associated detector/catalog modules, leaving the existing install/update/uninstall flows unchanged. Because phase 1 is read-only and phase 2 reuses existing install primitives, rollback is limited to removing the new command surface and tests rather than migrating user data.

## Dependencies

- Existing skill lifecycle modules in `src/skills/install.rs`, `src/skills/update.rs`, and `src/skills/uninstall.rs`
- Existing registry persistence in `src/skills/registry.rs`
- Existing interactive prompt precedent in `src/init.rs`
- Future spec/design work to define command names, JSON schema, and exact recommendation metadata structure

## Success Criteria

- [ ] A proposal-approved phase split exists between read-only suggestion and guided installation.
- [ ] Subsequent specs can define a deterministic local detection contract for Rust, TypeScript/Node, Astro, GitHub Actions, Docker, Make, and Python.
- [ ] Subsequent design can reuse current install/registry flows instead of introducing a parallel installer.
- [ ] The change leaves a clear evolution path toward provider-backed catalog metadata without making v1 depend on network access.
