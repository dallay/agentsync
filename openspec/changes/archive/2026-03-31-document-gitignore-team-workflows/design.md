# Design: Document Gitignore Team Workflows

## Technical Approach

This change is a docs-information-architecture update, not a product change. The implementation will add one canonical Starlight guide for gitignore/team workflows, then reduce other docs surfaces to short summaries that point back to that guide while still preserving their local reference value.

The design follows the proposal and the existing `openspec/specs/gitignore-management/spec.md` source of truth. The docs must describe current behavior accurately: managed mode remains the default, disabled mode is an opt-out, disabled mode cleans up the managed block on `apply`, `--no-gitignore` skips reconciliation entirely, and root-scoped managed entries are part of the current behavior.

## Architecture Decisions

### Decision: Use a single canonical workflow guide under `guides/`

**Choice**: Create a dedicated guide page at `website/docs/src/content/docs/guides/gitignore-team-workflows.mdx` and treat it as the only page that contains the full workflow narrative.

**Alternatives considered**: Expand the configuration reference section only; split content across getting-started, CLI, and configuration pages; add separate workflow pages per mode.

**Rationale**: The repo already uses Starlight guides for task-oriented content and reference pages for detailed command/config semantics. A single canonical guide best matches the exploration outcome, minimizes duplication, and gives other pages a stable place to link to.

### Decision: Keep supporting pages summary-oriented and context-specific

**Choice**: Update supporting pages to keep only the minimum content appropriate to their role: landing pages point to the guide, getting-started gives a short recommendation, configuration documents the fields and behavior, CLI documents command/flag semantics, and README surfaces link users to the guide instead of re-explaining it.

**Alternatives considered**: Duplicate the same workflow explanation verbatim everywhere; remove gitignore references from supporting pages entirely.

**Rationale**: Readers arrive from different entry points, so each page still needs a concise local explanation. However, duplicating the full workflow would quickly drift, especially with recent changes like disabled cleanup and root-scoped entries.

### Decision: Document Windows only as scoped notes, not parallel instructions

**Choice**: Mention Windows caveats only as short callouts where necessary and defer deep platform-specific setup or alternate step lists until DALLAY-220.

**Alternatives considered**: Add a separate Windows subsection to the canonical guide; duplicate every workflow step for Windows.

**Rationale**: The proposal explicitly calls out avoiding Windows-specific duplication before DALLAY-220. Small notes preserve accuracy without creating a second documentation tree to maintain.

## Data Flow

The documentation update will flow from spec-backed behavior into one canonical guide, then into concise references from supporting pages.

```text
gitignore-management spec
        │
        ▼
canonical guide (full workflow narrative)
        │
        ├──► docs index feature/link
        ├──► getting started summary + next step
        ├──► configuration reference field semantics + link
        ├──► CLI reference apply/--no-gitignore semantics + link
        ├──► root README quick guidance + link
        └──► npm README brief pointer + link
```

Reader flow should be:

```text
Entry page / README
    └──► canonical guide for team decision
              ├──► managed-gitignore workflow
              └──► opt-out committed-symlink workflow

Reference pages
    └──► precise field / flag semantics
              └──► link back to canonical guide for workflow selection
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `website/docs/src/content/docs/guides/gitignore-team-workflows.mdx` | Create | Canonical guide for team-facing gitignore workflows, command examples, and decision framing. |
| `website/docs/astro.config.mjs` | Modify | Add the new guide to the explicit Guides sidebar so it is discoverable in navigation. |
| `website/docs/src/content/docs/index.mdx` | Modify | Update landing-page gitignore feature/link to point to the canonical guide instead of only the configuration anchor. |
| `website/docs/src/content/docs/guides/getting-started.mdx` | Modify | Add a brief recommendation after `agentsync apply`/team setup and link to the new guide. |
| `website/docs/src/content/docs/reference/configuration.mdx` | Modify | Keep technical `[gitignore]` field documentation, but clarify default-vs-opt-out behavior, root-scoped entries, and disabled cleanup with a link to the guide. |
| `website/docs/src/content/docs/reference/cli.mdx` | Modify | Clarify that `apply` reconciles `.gitignore` according to config, explain `--no-gitignore` as strict opt-out, and link to the guide for workflow choice. |
| `README.md` | Modify | Shorten gitignore/team-workflow explanation to a brief summary and point repo readers to the canonical guide. |
| `npm/agentsync/README.md` | Modify | Add a compact pointer to the canonical guide for package users without duplicating the workflow content. |

## Interfaces / Contracts

No code interfaces or runtime contracts change. The operative documentation contract is:

1. `openspec/specs/gitignore-management/spec.md` remains the behavioral source of truth.
2. `guides/gitignore-team-workflows.mdx` becomes the documentation source of truth for team workflow guidance.
3. Supporting pages MUST summarize, not restate, workflow steps and MUST link to the canonical guide when discussing team choices.

Content contract for the new guide:

```md
## When to use each workflow
- Default managed-gitignore workflow (`[gitignore].enabled = true`)
- Opt-out committed-symlink workflow (`[gitignore].enabled = false`)

## Default managed-gitignore workflow
- What AgentSync writes to `.gitignore`
- Root-scoped managed entries behavior
- What teammates run
- Expected diffs / review expectations

## Opt-out committed-symlink workflow
- When to choose it
- Config change (`enabled = false`)
- Cleanup of stale managed block on `apply`
- Marker-aware preservation of unmanaged lines

## Temporary opt-out
- `agentsync apply --no-gitignore`
- Why this differs from disabling gitignore management in config

## Platform notes
- Brief Windows note only if required
```

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Content consistency | Canonical guide and supporting pages use the same terminology for default vs opt-out behavior | Manual review against `openspec/specs/gitignore-management/spec.md` and proposal success criteria |
| Docs build | New page, links, imports, and sidebar config compile correctly | Run `pnpm run docs:build` from repo root |
| Formatting / prose hygiene | Updated Markdown/MDX remains repository-style compliant and links are valid enough for site build | Run `pnpm run biome:check` if touched files fall under workspace-wide formatting checks; fix only if the edited files require it |
| Link/navigation sanity | Sidebar entry, landing links, and inline cross-links resolve to the new guide | Verify in built docs or local preview by checking generated navigation and page references |

## Command Examples To Include

The canonical guide should include short, copyable command snippets that map to the two workflows and the temporary CLI opt-out:

```bash
# Standard team workflow: create/update symlinks and managed .gitignore entries
agentsync apply

# Preview the same reconciliation without writing files
agentsync apply --dry-run

# Intentional opt-out workflow: commit managed destinations instead of ignoring them
# in .agents/agentsync.toml
[gitignore]
enabled = false

# Reconcile after disabling management; this removes the stale managed block
agentsync apply

# Temporary CLI-only opt-out: skip gitignore reconciliation for this run only
agentsync apply --no-gitignore

# Preview with temporary opt-out; should not report gitignore cleanup/write actions
agentsync apply --dry-run --no-gitignore
```

Supporting pages should reuse only the command snippets relevant to their scope:

- `getting-started.mdx`: `agentsync apply`
- `reference/configuration.mdx`: TOML snippets for `[gitignore].enabled = true/false`
- `reference/cli.mdx`: `apply`, `--dry-run`, `--no-gitignore`
- `README.md` and `npm/agentsync/README.md`: one summary example plus a link to the guide

## Migration / Rollout

No migration required.

Rollout is a docs-only change delivered in one pass:

1. Add the canonical guide.
2. Add sidebar/navigation discoverability.
3. Update supporting pages to summary + cross-link form.
4. Build docs and confirm no broken MDX/navigation issues.

## Open Questions

- [ ] Whether `README.md` should include a short dedicated “team workflows” subsection or only a single cross-link near existing `.gitignore` examples.
- [ ] Whether `npm/agentsync/README.md` should get just a docs link or a one-paragraph summary plus link.
