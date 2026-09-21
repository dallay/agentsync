# Evidence — clerk-testing

## Local state
- Path: /Users/acosta/Dev/dallay/agents-skills/skills/clerk-testing/
- Frontmatter (verbatim):
  ```
  name: clerk-testing
  description: E2E testing for Clerk apps. Use with Playwright or Cypress for auth flow
    tests.
  allowed-tools: WebFetch
  license: MIT
  metadata:
    author: clerk
    version: 1.2.0
  compatibility: Requires CLERK_TESTING_TOKEN from Clerk dashboard
  ```
- Bytes: 1931
- Has references/: N (no body-linked companions; only cross-skill pointers)
- SHA-256 (local): `0db14edb4d69fe703eb3cf7c2e99421e2283e49c5b1c35b3fd8a22827ae0afbf`
- Content excerpt (first 30 lines, no frontmatter):
  > # Testing
  > ## Decision Tree
  > | Framework | Documentation |

## Upstream search
- Search performed:
  - `gh api repos/clerk/skills/contents/skills/features/clerk-testing/SKILL.md` → fetched; upstream SHA-256 = `0db14edb4d69fe703eb3cf7c2e99421e2283e49c5b1c35b3fd8a22827ae0afbf` (blob SHA `46b394e040ab071b89059f2134b480f9f5836048`).
- Candidates found:
  - `clerk/skills@main` at commit `aac39ed99f18...` (HEAD, 2026-08-20). **Local bytes match upstream exactly** (same SHA-256).
- Match analysis: Byte-for-byte identical to upstream `main` HEAD. Frontmatter `version: 1.2.0` matches.

## License evidence
- License claimed in local frontmatter: MIT.
- License at upstream: per-file `license: MIT` in frontmatter, but **no repository-level LICENSE file** (`license: null`). No NOTICE, CONTRIBUTING, or COPYING.
- Immutable commit pin: `aac39ed99f18...` (upstream blob SHA `46b394e040ab071b89059f2134b480f9f5836048`).
- Maintainer permission: implicit via Clerk org + `npx skills add clerk/skills`. No explicit re-distribution grant located.

## Verdict
- **DEFER** (license-evidence gap; bytes are clean).
- Reason: Byte-identical to upstream `main` HEAD, but REQ-SKILLREC-009 requires authoritative MIT license + maintainer permission evidence. Per-file frontmatter `license: MIT` is not authoritative SPDX without a top-level LICENSE file. No companion references declared by this skill, so vendoring gap is minimal.
- Frontmatter patch required (hypothetical, if acceptance is later approved):
  ```
  metadata:
    author: clerk
    version: 1.2.0
    source: clerk/skills
    source_commit: aac39ed99f18...<full 40-hex SHA>
  ```

## Open
- Whether Clerk will add a top-level LICENSE file (this is the gating open question across all 8 clerk-* skills).
