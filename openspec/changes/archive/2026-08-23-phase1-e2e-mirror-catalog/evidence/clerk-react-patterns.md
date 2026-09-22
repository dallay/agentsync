# Evidence — clerk-react-patterns

## Local state
- Path: /Users/acosta/Dev/dallay/agents-skills/skills/clerk-react-patterns/
- Frontmatter (verbatim):
  ```
  name: clerk-react-patterns
  description: 'React SPA auth patterns with @clerk/react for Vite/CRA - ClerkProvider
    setup, useAuth/useUser/useClerk hooks, React Router protected routes, custom sign-in
    flows. Triggers on: Vite Clerk setup, React Router auth, useAuth hook, protected
    route, custom sign-in form React.'
  license: MIT
  allowed-tools: WebFetch
  metadata:
    author: clerk
    version: 1.0.0
  ```
- Bytes: 4162
- Has references/: N (body links to `references/hooks.md`, `references/protected-routes.md`, `references/custom-flows.md`, `references/router-integration.md` — none present)
- SHA-256 (local): `ab22780dd32403d2f4c5e6979a5cc0e721215ea9ec9b1f3e61b49b232c47b251`
- Content excerpt (first 30 lines, no frontmatter):
  > # React SPA Patterns
  > > This skill covers `@clerk/react` for Vite/CRA SPAs...

## Upstream search
- Search performed:
  - `gh api repos/clerk/skills/contents/skills/frameworks/clerk-react-patterns/SKILL.md` → fetched; upstream SHA-256 = `ab22780dd32403d2f4c5e6979a5cc0e721215ea9ec9b1f3e61b49b232c47b251` (blob SHA `84496131d6db5129e4083a841802b2fbaf3ceb78`).
- Candidates found:
  - `clerk/skills@main` at commit `aac39ed99f18...` (HEAD, 2026-08-20). **Local bytes match upstream exactly** (same SHA-256).
- Match analysis: Byte-for-byte identical to upstream `main` HEAD. Frontmatter `version: 1.0.0` matches.

## License evidence
- License claimed in local frontmatter: MIT.
- License at upstream: per-file `license: MIT` in frontmatter, but **no repository-level LICENSE file** (`license: null`). No NOTICE, CONTRIBUTING, or COPYING.
- Immutable commit pin: `aac39ed99f18...` (upstream blob SHA `84496131d6db5129e4083a841802b2fbaf3ceb78`).
- Maintainer permission: implicit via Clerk org + `npx skills add clerk/skills` documented in upstream `README.md`. No explicit re-distribution grant located.

## Verdict
- **DEFER** (license-evidence gap; bytes are clean).
- Reason: Byte-identical to upstream `main` HEAD, but REQ-SKILLREC-009 requires authoritative MIT license + maintainer permission evidence. Per-file frontmatter `license: MIT` is not authoritative SPDX without a top-level LICENSE file. The 4 missing companion `references/*.md` files must be vendored if accepted.
- Frontmatter patch required (hypothetical, if acceptance is later approved):
  ```
  metadata:
    author: clerk
    version: 1.0.0
    source: clerk/skills
    source_commit: aac39ed99f18...<full 40-hex SHA>
  ```
  AND the 4 `references/*.md` files must be vendored.

## Open
- Whether Clerk will add a top-level LICENSE file.
- Whether vendoring the 4 missing companion files is in scope before acceptance.
