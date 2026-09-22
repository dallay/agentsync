# Evidence — clerk-custom-ui

## Local state
- Path: /Users/acosta/Dev/dallay/agents-skills/skills/clerk-custom-ui/
- Frontmatter (verbatim):
  ```
  name: clerk-custom-ui
  description: Custom authentication flows and component appearance - hooks (useSignIn,
    useSignUp), themes, colors, fonts, CSS. Use for custom sign-in/sign-up flows, appearance
    styling, visual customization, branding.
  allowed-tools: WebFetch
  license: MIT
  metadata:
    author: clerk
    version: 2.3.0
  ```
- Bytes: 6494
- Has references/: N (body links to `core-2/custom-sign-in.md`, `core-2/custom-sign-up.md`, `core-3/custom-sign-in.md`, `core-3/custom-sign-up.md`, `core-3/show-component.md` — none present; these are Core 2 / Core 3 SDK reference files, not vendored)
- SHA-256 (local): `16398a39c8241e51f05cf6465deceb6d551996e1e0164ded597d43de9be42038`
- Content excerpt (first 30 lines, no frontmatter):
  > # Custom UI
  > > **Prerequisite**: Ensure `ClerkProvider` wraps your app...

## Upstream search
- Search performed:
  - `gh api repos/clerk/skills/contents/skills/core/clerk-custom-ui/SKILL.md` → fetched; upstream SHA-256 = `16398a39c8241e51f05cf6465deceb6d551996e1e0164ded597d43de9be42038` (blob SHA `e6e05dc90c169881b4e4241671d1fd721a26fe62`).
- Candidates found:
  - `clerk/skills@main` at commit `aac39ed99f18...` (HEAD, 2026-08-20). **Local bytes match upstream exactly** (same SHA-256).
- Match analysis: Byte-for-byte identical to upstream `main` HEAD. Frontmatter `version: 2.3.0` matches. The body declares a `core-2/` and `core-3/` companion directory structure that is NOT vendored locally — the upstream `clerk/skills` repo also does not currently ship these as `references/` (they would need to be added separately by Clerk, or the local body is forward-looking content).
- License evidence gap is the same as the other Clerk skills.

## License evidence
- License claimed in local frontmatter: MIT.
- License at upstream: per-file `license: MIT` in frontmatter, but **no repository-level LICENSE file** (`license: null`). No NOTICE, CONTRIBUTING, or COPYING.
- Immutable commit pin: `aac39ed99f18...` (upstream blob SHA `e6e05dc90c169881b4e4241671d1fd721a26fe62`).
- Maintainer permission: implicit via Clerk org + `npx skills add clerk/skills`. No explicit re-distribution grant located.

## Verdict
- **DEFER** (license-evidence gap; bytes are clean; companion gap).
- Reason: Byte-identical to upstream `main` HEAD, but REQ-SKILLREC-009 requires authoritative MIT license + maintainer permission evidence. Per-file frontmatter `license: MIT` is not authoritative SPDX without a top-level LICENSE file. The body declares `core-2/` and `core-3/` subdirectory references that are not vendored locally — these are SKILL.md-level companion references that may or may not exist upstream yet, so vendoring them is non-trivial.
- Frontmatter patch required (hypothetical, if acceptance is later approved):
  ```
  metadata:
    author: clerk
    version: 2.3.0
    source: clerk/skills
    source_commit: aac39ed99f18...<full 40-hex SHA>
  ```
  AND the 5 `core-2/` + `core-3/` companion files must be vendored (if they exist upstream) or replaced with inline content.

## Open
- Whether Clerk will add a top-level LICENSE file (gating question).
- Whether the `core-2/` + `core-3/` companion files exist upstream and can be vendored.
