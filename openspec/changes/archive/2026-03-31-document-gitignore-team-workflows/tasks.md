# Tasks: Document Gitignore Team Workflows

## Phase 1: Canonical Workflow Guide

- [x] 1.1 Create `website/docs/src/content/docs/guides/gitignore-team-workflows.mdx` as the single source of truth for team workflow guidance, with decision framing for default managed mode vs opt-out committed-symlink mode.
- [x] 1.2 In that guide, add step-by-step maintainer and collaborator instructions for both workflows, including expected diffs after `agentsync init`/`agentsync apply`, prepare-hook guidance, and when teams should choose each mode.
- [x] 1.3 Document current behavior in the guide with copyable examples: `[gitignore].enabled = true` as default, root-scoped managed entries, disabled cleanup of stale managed blocks, `agentsync apply --no-gitignore`, and a brief link-oriented Windows note.
- [x] 1.4 Update `website/docs/astro.config.mjs` so the new guide appears in the Guides navigation and is easy to discover from docs sidebar flows.

## Phase 2: Supporting Docs Alignment

- [x] 2.1 Update `website/docs/src/content/docs/reference/configuration.mdx` to keep `[gitignore]` field semantics technical while clearly stating enabled is the default, disabled is an intentional opt-out, cleanup is marker-aware, and full workflow details live in the guide.
- [x] 2.2 Update `website/docs/src/content/docs/reference/cli.mdx` to explain `agentsync apply`, `--dry-run`, and `--no-gitignore` in workflow terms, especially that `--no-gitignore` skips reconciliation for that invocation only, and link to the guide.
- [x] 2.3 Update `website/docs/src/content/docs/index.mdx` and `website/docs/src/content/docs/guides/getting-started.mdx` with short recommendation-oriented summaries that point readers to the canonical guide instead of duplicating workflow steps.
- [x] 2.4 Update `README.md` and `npm/agentsync/README.md` with one concise team-workflow summary plus a link to the canonical guide, keeping wording consistent with the default-vs-opt-out framing.

## Phase 3: Verification And Consistency Review

- [x] 3.1 Review all edited docs against `openspec/specs/gitignore-management/spec.md` and `openspec/changes/document-gitignore-team-workflows/specs/documentation/spec.md` to confirm terminology, collaborator expectations, cleanup behavior, and root-scoped entry details stay aligned.
- [x] 3.2 Run `pnpm run docs:build` from the repo root and fix any MDX, sidebar, or broken-link issues introduced by the new guide and cross-links.
- [x] 3.3 Run `pnpm exec biome check README.md npm/agentsync/README.md website/docs/src/content/docs/index.mdx website/docs/src/content/docs/guides/getting-started.mdx website/docs/src/content/docs/guides/gitignore-team-workflows.mdx website/docs/src/content/docs/reference/configuration.mdx website/docs/src/content/docs/reference/cli.mdx` if formatting or prose lint drift appears in touched files.
