# Proposal: Document Windows Symlink Setup

## Intent

DALLAY-220 needs a dedicated, linkable Windows setup guide so teams adopting AgentSync can resolve
Windows-specific symlink prerequisites without bloating the shared workflow docs. DALLAY-219
intentionally kept Windows details minimal in the canonical team-workflows guide, which leaves a gap
for readers who need actionable Windows setup help and for other docs surfaces that need a stable
destination to reference.

## Scope

### In Scope\n\n- Add a dedicated Windows-focused documentation page that explains how to prepare a Windows environment for AgentSync symlink usage.

- Define the target information architecture so shared workflow and setup docs can link to the
  Windows page instead of embedding deep platform-specific instructions.
- Update the main documentation entry points that mention Windows, symlinks, onboarding, or workflow
  setup so they consistently cross-link to the dedicated guide.
- Preserve the current product defaults and keep general team workflow guidance platform-neutral.

### Out of Scope\n\n- Changing AgentSync runtime behavior, symlink strategy, or default gitignore/workflow policy.

- Rewriting the general team workflow guide into separate OS-specific variants.
- Providing a broad Windows troubleshooting catalog beyond the setup guidance needed for successful
  AgentSync adoption.
- Reorganizing the full docs site beyond the pages needed for the new guide and supporting
  cross-links.

## Approach

Create one dedicated Windows symlink setup guide under `website/docs/src/content/docs/guides/` and
treat it as the canonical destination for Windows-specific prerequisites and onboarding notes. The
guide should focus on platform setup topics such as Windows symlink permissions/prerequisites,
practical setup expectations, validation cues, and pointers back to the existing shared workflow
documentation for the cross-platform maintainer/collaborator flow.

The target docs IA should be:

1. **Shared workflow/source-of-truth docs stay cross-platform**
    - `website/docs/src/content/docs/guides/gitignore-team-workflows.mdx`
    - `website/docs/src/content/docs/guides/getting-started.mdx`
    - `website/docs/src/content/docs/reference/configuration.mdx`
    - `website/docs/src/content/docs/reference/cli.mdx`

   These pages should keep Windows content brief and link outward when platform-specific setup is
   needed.

2. **Dedicated platform page becomes the canonical Windows destination**
    - `website/docs/src/content/docs/guides/windows-symlink-setup.mdx` (new)

   This page should answer “What does a Windows user need to do before AgentSync symlinks work
   reliably?” without restating the whole team workflow narrative.

3. **Cross-link surfaces point to the canonical Windows guide**
    - docs homepage / guide navigation
    - getting started
    - gitignore team workflows platform note
    - CLI/config/reference surfaces where Windows symlink caveats are mentioned
    - repository README and `npm/agentsync/README.md` where Windows symlink setup is currently
      referenced or implied

This keeps Windows content platform-specific rather than duplicating the broader workflow docs from
DALLAY-219.

## Affected Areas

| Area                                                                | Impact   | Description                                                                               |
|---------------------------------------------------------------------|----------|-------------------------------------------------------------------------------------------|
| `website/docs/src/content/docs/guides/windows-symlink-setup.mdx`    | New      | Dedicated Windows symlink setup guide and canonical link target.                          |
| `website/docs/astro.config.mjs`                                     | Modified | Add the new guide to navigation if appropriate for guide discoverability.                 |
| `website/docs/src/content/docs/guides/gitignore-team-workflows.mdx` | Modified | Replace the generic Windows note with a link to the dedicated setup guide.                |
| `website/docs/src/content/docs/guides/getting-started.mdx`          | Modified | Add a Windows-specific cross-link near setup/apply guidance.                              |
| `website/docs/src/content/docs/index.mdx`                           | Modified | Surface the Windows setup guide from a primary entry point if needed.                     |
| `website/docs/src/content/docs/reference/configuration.mdx`         | Modified | Link to the Windows guide when field behavior intersects with Windows setup expectations. |
| `website/docs/src/content/docs/reference/cli.mdx`                   | Modified | Link to the Windows guide where `apply`/symlink behavior may need platform setup context. |
| `README.md`                                                         | Modified | Update the existing Windows symlink troubleshooting note to point to the dedicated guide. |
| `npm/agentsync/README.md`                                           | Modified | Add a concise Windows setup link from the npm-facing onboarding surface.                  |

## Risks

| Risk                                                                                    | Likelihood | Mitigation                                                                                                             |
|-----------------------------------------------------------------------------------------|------------|------------------------------------------------------------------------------------------------------------------------|
| Windows guidance duplicates the team workflow guide and creates two sources of truth    | Medium     | Keep the new page focused on Windows prerequisites/setup and link back to shared workflow docs for policy and process. |
| Docs imply Windows requires a different product default or workflow policy              | Low        | Reiterate that product defaults stay unchanged and platform setup only affects environment readiness.                  |
| Cross-links are added inconsistently, leaving some readers on stale or minimal guidance | Medium     | Update all high-traffic entry points in one rollout and verify link coverage during docs review.                       |
| Windows instructions drift from actual supported setup expectations over time           | Medium     | Keep the guide scoped to stable prerequisites and centralize Windows-specific wording in one canonical page.           |

## Rollback Plan

If the new guide proves inaccurate or too duplicative, revert the new Windows guide and related
cross-link edits, returning docs surfaces to their prior brief Windows notes. Because this is a
docs-only change, rollback is limited to documentation/navigation updates and does not require
product or config migration.

## Dependencies

- Existing canonical workflow guide from DALLAY-219 at
  `website/docs/src/content/docs/guides/gitignore-team-workflows.mdx`.
- Current documentation navigation and entry-point structure under `website/docs/src/content/docs/`
  and `website/docs/astro.config.mjs`.
- Existing Windows troubleshooting mention in `README.md` as a seed for the new canonical link
  target.
- Documentation validation via `pnpm run docs:build` during implementation.

## Success Criteria

- [ ] The docs include a dedicated Windows symlink setup guide that other docs can link to directly.
- [ ] Shared workflow docs keep Windows content brief and platform-specific details live in the
  dedicated Windows guide instead of a duplicated workflow section.
- [ ] Getting started, workflow, reference, and README surfaces consistently cross-link to the
  Windows guide where Windows symlink setup is relevant.
- [ ] The documentation clearly states or implies that AgentSync product defaults remain unchanged
  and that the change is documentation-only.
- [ ] A Windows reader can find actionable setup guidance from at least one primary entry point
  without needing to infer steps from scattered troubleshooting notes.
