# Tasks: Document Windows Symlink Setup

## Phase 1: Canonical Guide Foundation

- [x] 1.1 Create `website/docs/src/content/docs/guides/windows-symlink-setup.mdx` with the guide outline and canonical framing, keeping scope limited to Windows setup/readiness rather than workflow policy.
- [x] 1.2 Fill the new guide with native Windows prerequisite guidance, WSL-as-optional-path positioning, actionable verification commands, recovery steps, and mixed-platform maintainer notes with links back to `/guides/gitignore-team-workflows/`.
- [x] 1.3 Update `website/docs/astro.config.mjs` to add `guides/windows-symlink-setup` in the Guides sidebar near setup/onboarding pages so the guide is discoverable from primary docs navigation.

## Phase 2: Docs Cross-link Integration

- [x] 2.1 Update `website/docs/src/content/docs/guides/gitignore-team-workflows.mdx` to replace any substantive Windows setup note with a brief link-oriented callout to `/guides/windows-symlink-setup/`.
- [x] 2.2 Update `website/docs/src/content/docs/guides/getting-started.mdx` and `website/docs/src/content/docs/index.mdx` to add concise Windows setup links from primary onboarding and docs entry surfaces.
- [x] 2.3 Update `website/docs/src/content/docs/reference/cli.mdx` and `website/docs/src/content/docs/reference/configuration.mdx` so Windows-specific symlink/setup caveats stay brief and point to the canonical guide.
- [x] 2.4 Update `README.md` and `npm/agentsync/README.md` to replace or add short Windows setup references that link to the new guide instead of embedding workflow detail.

## Phase 3: Review And Validation

- [x] 3.1 Review all touched pages to confirm Windows content is centralized in `guides/windows-symlink-setup.mdx`, shared workflow pages remain platform-neutral, and links consistently use `/guides/windows-symlink-setup/`.
- [x] 3.2 Run `pnpm run docs:build` from the repository root and fix any MDX, sidebar, or internal-link issues surfaced by the build.
- [x] 3.3 Do a final diff check to confirm repo README and npm README wording matches the docs guide boundaries and does not imply Windows-specific product defaults or a separate team workflow.
