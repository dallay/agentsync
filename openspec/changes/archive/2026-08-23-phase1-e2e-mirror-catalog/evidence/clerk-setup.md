# Evidence — clerk-setup

## Local state
- Path: /Users/acosta/Dev/dallay/agents-skills/skills/clerk-setup/
- Frontmatter (verbatim):
  ```
  name: clerk-setup
  description: Add Clerk authentication to any project by following the official quickstart
    guides.
  license: MIT
  allowed-tools: WebFetch
  compatibility: Requires NEXT_PUBLIC_CLERK_PUBLISHABLE_KEY and CLERK_SECRET_KEY (or framework-specific equivalents like VITE_CLERK_PUBLISHABLE_KEY for Vite-based apps). Keys can be auto-generated via Keyless on first SDK initialization, or pulled from the Clerk Dashboard. Requires Node.js 20.9.0 or higher.
  metadata:
    author: clerk
    version: 2.3.0
  ```
- Bytes: 13042
- Has references/: N (none)
- SHA-256 (local): `2357d8a7e8a1a6e01c4d3ba31d5bedadbeef9f44b969cf311107831994b91b5e`
- Content excerpt (first 30 lines, no frontmatter):
  > # Adding Clerk
  > > **Version**: Check `package.json` for the SDK version — see `clerk` skill for the version table...
  > This skill sets up Clerk for authentication by following the official quickstart documentation...

## Upstream search
- Search performed:
  - `gh search repos "clerk skills" --limit 10` → `clerk/skills` is the public official Clerk organization repo (67 stars, 4 forks, created 2026-01-06, default_branch=`main`, archived=false, fork=false, license=**null**).
  - `gh api repos/clerk/skills/license` → 404 Not Found.
  - `gh api repos/clerk/skills/contents/LICENSE` → 404.
  - `gh api repos/clerk/skills/contents/LICENSE.md` → 404.
  - `gh api repos/clerk/skills/contents/COPYING` → 404.
  - `gh api repos/clerk/skills/contents/NOTICE` → 404.
  - `gh api repos/clerk/skills/contents/CONTRIBUTING` and `CONTRIBUTING.md` → 404.
  - `gh api repos/clerk/skills/contents/AGENTS.md` → no license discussion; only structural/contribution guidance.
  - `gh api repos/clerk/skills/contents/skills` → skills dir contains `core/`, `features/`, `frameworks/`, `mobile/`.
  - `gh api repos/clerk/skills/contents/skills/core/clerk-setup/SKILL.md` → fetched; SHA-256 mismatch with local (different `version: 2.5.0` upstream vs `2.3.0` local; "Keyless" wording replaced by "no login required / temporary dev keys" in upstream).
- Candidates found:
  - `clerk/skills` at commit `aac39ed99f18...` (latest main HEAD, 2026-08-20). Local bytes do NOT match (different version, different prose).
  - Earlier upstream commit (pre-2026-08-20) may match `v2.3.0`; not located in this audit pass.
- Match analysis: Content is byte-different from latest upstream main. Earlier upstream commit may match; not pinpointed.

## License evidence
- License claimed in local frontmatter: MIT (yes, declared).
- License at upstream: **No top-level LICENSE file, no SPDX metadata** (`gh api repos/clerk/skills` returns `license: null`). Each upstream SKILL.md declares `license: MIT` in its own YAML frontmatter only (consistent with local), but there is no repository-level license grant or copyright notice.
- Immutable commit pin: cannot pin to `aac39ed99f18` (latest) because bytes diverge. Older upstream commit carrying `clerk-setup` at version `2.3.0` must be located for a true byte-pinning.
- Maintainer permission: implicit only via the public Clerk GitHub org distribution of the `clerk/skills` repo and the `npx skills add clerk/skills` install path documented in upstream `README.md`. No explicit "you may re-host / mirror / fork" notice located.

## Verdict
- **DEFER** (license-evidence gap + version drift).
- Reason: Even though the content is recognizably the same skill authored by Clerk, the upstream repo has no repository-level LICENSE file or SPDX identifier. REQ-SKILLREC-009 requires "authoritative MIT license + maintainer permission evidence at an immutable commit". Per-file frontmatter `license: MIT` is not authoritative SPDX, and content bytes have drifted from upstream `main`. Re-pull at a pinned commit matching local bytes, OR file an explicit Clerk permission grant, before accepting.
- Frontmatter patch required (hypothetical, if a future acceptance is approved):
  ```
  metadata:
    author: clerk
    version: 2.3.0
    source: clerk/skills
    source_commit: <pinned-40-hex-SHA where local bytes match>
    references_status: pending-vendoring
  ```
  Companion `references/*.md` files referenced in the body (none in this skill — this is the entry-point) do not exist on disk.

## Open
- Exact upstream commit SHA where `clerk-setup` SKILL.md was at version 2.3.0 with identical bytes to local.
- Whether Clerk org will add a top-level LICENSE file or provide a written re-distribution grant.
- Companion references inventory from upstream `clerk/skills` that the body links to (none declared by this skill — the only `references/*.md` references in clerk-setup body are cross-skill pointers like `clerk-custom-ui`, `clerk-nextjs-patterns`, etc., not local reference files).
