# Evidence — clerk-vue-patterns

## Local state
- Path: /Users/acosta/Dev/dallay/agents-skills/skills/clerk-vue-patterns/
- Frontmatter (verbatim):
  ```
  name: clerk-vue-patterns
  description: 'Vue 3 patterns with Clerk — composables (useAuth, useUser,
    useClerk, useOrganization), Vue Router guards, Pinia auth store
    integration. Triggers on: vue clerk, useAuth vue, clerk composables,
    vue router clerk guard, pinia auth clerk. For Nuxt, use clerk-nuxt-patterns instead.'
  license: MIT
  allowed-tools: WebFetch
  metadata:
    author: clerk
    version: 1.0.0
  ```
- Bytes: 2833
- Has references/: N (body links to `references/composables.md`, `references/vue-router-guards.md`, `references/pinia-integration.md` — none present)
- SHA-256 (local): `626fca491d4f46c8b8849015e70fe25b89d0f80b55b762c76696c2f0a916699c`
- Content excerpt (first 30 lines, no frontmatter):
  > # Vue Patterns
  > SDK: `@clerk/vue` v2+ (Vue 3). For Nuxt, use `clerk-nuxt-patterns`...

## Upstream search
- Search performed:
  - `gh api repos/clerk/skills/contents/skills/frameworks/clerk-vue-patterns/SKILL.md` → fetched; upstream SHA-256 = `626fca491d4f46c8b8849015e70fe25b89d0f80b55b762c76696c2f0a916699c` (blob SHA `0109b3d7a2b6cc34fc6551753fdb1d3834035ddf`).
- Candidates found:
  - `clerk/skills@main` at commit `aac39ed99f18...` (HEAD, 2026-08-20). **Local bytes match upstream exactly** (same SHA-256).
- Match analysis: Byte-for-byte identical to upstream `main` HEAD. Frontmatter `version: 1.0.0` matches.

## License evidence
- License claimed in local frontmatter: MIT.
- License at upstream: per-file `license: MIT` in frontmatter, but **no repository-level LICENSE file** (`license: null`). No NOTICE, CONTRIBUTING, or COPYING.
- Immutable commit pin: `aac39ed99f18...` (upstream blob SHA `0109b3d7a2b6cc34fc6551753fdb1d3834035ddf`).
- Maintainer permission: implicit via Clerk org + `npx skills add clerk/skills`. No explicit re-distribution grant located.

## Verdict
- **DEFER** (license-evidence gap; bytes are clean).
- Reason: Byte-identical to upstream `main` HEAD, but REQ-SKILLREC-009 requires authoritative MIT license + maintainer permission evidence. Per-file frontmatter `license: MIT` is not authoritative SPDX without a top-level LICENSE file. The 3 missing companion `references/*.md` files must be vendored if accepted.
- Frontmatter patch required (hypothetical, if acceptance is later approved):
  ```
  metadata:
    author: clerk
    version: 1.0.0
    source: clerk/skills
    source_commit: aac39ed99f18...<full 40-hex SHA>
  ```
  AND the 3 `references/*.md` files must be vendored.

## Open
- Whether Clerk will add a top-level LICENSE file.
- Whether vendoring the 3 missing companion files is in scope before acceptance.
