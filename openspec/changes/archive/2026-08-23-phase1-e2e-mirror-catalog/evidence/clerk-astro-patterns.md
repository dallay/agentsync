# Evidence — clerk-astro-patterns

## Local state
- Path: /Users/acosta/Dev/dallay/agents-skills/skills/clerk-astro-patterns/
- Frontmatter (verbatim):
  ```
  name: clerk-astro-patterns
  description: 'Astro patterns with Clerk — middleware, SSR pages, island components,
    API routes, static vs SSR rendering. Triggers on: astro clerk, clerk astro middleware,
    astro protected page, clerk island component, astro API route auth, clerk astro
    SSR.'
  license: MIT
  allowed-tools: WebFetch
  metadata:
    author: clerk
    version: 1.0.0
  ```
- Bytes: 3305
- Has references/: N (body links to `references/middleware.md`, `references/ssr-pages.md`, `references/island-components.md`, `references/api-routes.md`, `references/astro-react.md` — none present)
- SHA-256 (local): `483df69ccaa6447a28f2757c1c576919fdf5807a19352ea6171a766c71098d08`
- Content excerpt (first 30 lines, no frontmatter):
  > # Astro Patterns
  > SDK: `@clerk/astro` v3+. Requires Astro 4.15+...

## Upstream search
- Search performed:
  - `gh api repos/clerk/skills/contents/skills/frameworks/clerk-astro-patterns/SKILL.md` → fetched; upstream SHA-256 = `483df69ccaa6447a28f2757c1c576919fdf5807a19352ea6171a766c71098d08` (blob SHA `0e5f731ecc8fcf69f721a07ab167a0265d35a6ea`).
- Candidates found:
  - `clerk/skills@main` at commit `aac39ed99f18...` (HEAD, 2026-08-20). **Local bytes match upstream exactly** (same SHA-256).
- Match analysis: Byte-for-byte identical to upstream `main` HEAD. Frontmatter `version: 1.0.0` matches.

## License evidence
- License claimed in local frontmatter: MIT.
- License at upstream: per-file `license: MIT` in frontmatter, but **no repository-level LICENSE file** (`license: null`). No NOTICE, CONTRIBUTING, or COPYING.
- Immutable commit pin: `aac39ed99f18...` (upstream blob SHA `0e5f731ecc8fcf69f721a07ab167a0265d35a6ea`).
- Maintainer permission: implicit via Clerk org + `npx skills add clerk/skills`. No explicit re-distribution grant located.

## Verdict
- **DEFER** (license-evidence gap; bytes are clean).
- Reason: Byte-identical to upstream `main` HEAD, but REQ-SKILLREC-009 requires authoritative MIT license + maintainer permission evidence. Per-file frontmatter `license: MIT` is not authoritative SPDX without a top-level LICENSE file. The 5 missing companion `references/*.md` files must be vendored if accepted.
- Frontmatter patch required (hypothetical, if acceptance is later approved):
  ```
  metadata:
    author: clerk
    version: 1.0.0
    source: clerk/skills
    source_commit: aac39ed99f18...<full 40-hex SHA>
  ```
  AND the 5 `references/*.md` files must be vendored.

## Open
- Whether Clerk will add a top-level LICENSE file.
- Whether vendoring the 5 missing companion files is in scope before acceptance.
