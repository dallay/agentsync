# Evidence — clerk-nextjs-patterns

## Local state
- Path: /Users/acosta/Dev/dallay/agents-skills/skills/clerk-nextjs-patterns/
- Frontmatter (verbatim):
  ```
  name: clerk-nextjs-patterns
  description: Advanced Next.js patterns - middleware, Server Actions, caching with
    Clerk.
  license: MIT
  allowed-tools: WebFetch
  compatibility: Requires NEXT_PUBLIC_CLERK_PUBLISHABLE_KEY and CLERK_SECRET_KEY. For manual JWT verification (standalone API servers without Clerk middleware), additionally requires CLERK_JWT_KEY or CLERK_PEM_PUBLIC_KEY.
  metadata:
    author: clerk
    version: 2.2.0
  ```
- Bytes: 8302
- Has references/: N (body links to `references/server-vs-client.md`, `references/middleware-strategies.md`, `references/server-actions.md`, `references/api-routes.md`, `references/caching-auth.md` — none present)
- SHA-256 (local): `51a833e5f5edab03b5632b1da1a7587fe1d812fc149e3acc69f59cccd39369e3`
- Content excerpt (first 30 lines, no frontmatter):
  > # Next.js Patterns
  > > **Version**: Check `package.json` for the SDK version...
  > For basic setup, see `clerk-setup` skill.

## Upstream search
- Search performed:
  - `gh api repos/clerk/skills/contents/skills/frameworks/clerk-nextjs-patterns/SKILL.md` → fetched; SHA-256 of upstream content = `51a833e5f5edab03b5632b1da1a7587fe1d812fc149e3acc69f59cccd39369e3` (blob SHA `7a2c0d7c60971c9d8ab9ded0d81e92aab86e9160`).
- Candidates found:
  - `clerk/skills@main` at commit `aac39ed99f18...` (HEAD, 2026-08-20). **Local bytes match upstream exactly** (same SHA-256).
- Match analysis: Byte-for-byte identical to upstream `main` HEAD. Frontmatter `version: 2.2.0` matches.

## License evidence
- License claimed in local frontmatter: MIT.
- License at upstream: per-file `license: MIT` in frontmatter, but **no repository-level LICENSE file** (`license: null` in `gh api repos/clerk/skills`). No NOTICE, no CONTRIBUTING, no COPYING.
- Immutable commit pin: `aac39ed99f18...` (full SHA available on `gh api repos/clerk/skills/commits/main`). The upstream blob SHA at this commit for the SKILL.md is `7a2c0d7c60971c9d8ab9ded0d81e92aab86e9160`.
- Maintainer permission: implicit only via the public Clerk GitHub org distribution and the upstream `README.md` documenting `npx skills add clerk/skills`. No explicit "you may re-host / mirror / fork" notice located.

## Verdict
- **DEFER** (license-evidence gap; bytes are clean).
- Reason: The content is byte-identical to upstream `main` HEAD, but REQ-SKILLREC-009 requires authoritative MIT license + maintainer permission evidence. Per-file `license: MIT` is not authoritative SPDX without a top-level LICENSE file. Companion references (`references/server-vs-client.md`, etc.) declared in body are NOT vendored locally. Accepting without SPDX evidence or vendoring companions would ship a broken / under-attributed skill.
- Frontmatter patch required (hypothetical, if acceptance is later approved):
  ```
  metadata:
    author: clerk
    version: 2.2.0
    source: clerk/skills
    source_commit: aac39ed99f18...<full 40-hex SHA>
  ```
  AND the five `references/*.md` files must be vendored from upstream `skills/frameworks/clerk-nextjs-patterns/references/` and added to `PROVENANCE.md`.

## Open
- Whether Clerk org will add a top-level LICENSE file at the repo root.
- Whether to vendoring the 5 missing companion files (~50KB total) before accepting.
- Whether a written re-distribution grant can be obtained from Clerk.
