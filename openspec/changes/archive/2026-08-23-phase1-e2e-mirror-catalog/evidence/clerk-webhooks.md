# Evidence — clerk-webhooks

## Local state
- Path: /Users/acosta/Dev/dallay/agents-skills/skills/clerk-webhooks/
- Frontmatter (verbatim):
  ```
  name: clerk-webhooks
  description: Clerk webhooks for real-time events and data syncing. Verify with verifyWebhook
    from the framework-specific package. Handle user, session, organization, billing, and
    payment events. Build event-driven features like database sync, notifications, and
    integrations.
  allowed-tools: WebFetch
  license: MIT
  metadata:
    author: clerk
    version: 1.2.0
  compatibility: Requires CLERK_WEBHOOK_SIGNING_SECRET (svix signing secret from Clerk dashboard)
  ```
- Bytes: 13562
- Has references/: N (body links to `references/frameworks.md` — not present; also references `clerk-cli`, `clerk-setup`, `clerk-orgs`, `clerk-billing`, `clerk-backend-api` as cross-skill pointers)
- SHA-256 (local): `99138f2fc461c25ce7181f805aa785ae3d8219c3f125e02870593280d7be9ad3`
- Content excerpt (first 30 lines, no frontmatter):
  > # Webhooks
  > Output complete, working webhook handlers with `verifyWebhook(req)` verification...

## Upstream search
- Search performed:
  - `gh api repos/clerk/skills/contents/skills/features/clerk-webhooks/SKILL.md` → fetched; upstream SHA-256 = `99138f2fc461c25ce7181f805aa785ae3d8219c3f125e02870593280d7be9ad3` (blob SHA `259f099f27d5c18cb305977514878cfc8c13ffa5`).
- Candidates found:
  - `clerk/skills@main` at commit `aac39ed99f18...` (HEAD, 2026-08-20). **Local bytes match upstream exactly** (same SHA-256).
- Match analysis: Byte-for-byte identical to upstream `main` HEAD. Frontmatter `version: 1.2.0` matches.

## License evidence
- License claimed in local frontmatter: MIT.
- License at upstream: per-file `license: MIT` in frontmatter, but **no repository-level LICENSE file** (`license: null`). No NOTICE, CONTRIBUTING, or COPYING.
- Immutable commit pin: `aac39ed99f18...` (upstream blob SHA `259f099f27d5c18cb305977514878cfc8c13ffa5`).
- Maintainer permission: implicit via Clerk org + `npx skills add clerk/skills`. No explicit re-distribution grant located.

## Verdict
- **DEFER** (license-evidence gap; bytes are clean).
- Reason: Byte-identical to upstream `main` HEAD, but REQ-SKILLREC-009 requires authoritative MIT license + maintainer permission evidence. Per-file frontmatter `license: MIT` is not authoritative SPDX without a top-level LICENSE file. The `references/frameworks.md` companion must be vendored if accepted.
- Frontmatter patch required (hypothetical, if acceptance is later approved):
  ```
  metadata:
    author: clerk
    version: 1.2.0
    source: clerk/skills
    source_commit: aac39ed99f18...<full 40-hex SHA>
  ```
  AND `references/frameworks.md` must be vendored.

## Open
- Whether Clerk will add a top-level LICENSE file.
- Whether vendoring `references/frameworks.md` is in scope before acceptance.
