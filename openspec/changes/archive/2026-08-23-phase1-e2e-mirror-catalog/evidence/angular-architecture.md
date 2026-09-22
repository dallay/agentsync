# Evidence — angular-architecture

## Local state
- Path: /Users/acosta/Dev/dallay/agents-skills/skills/angular-architecture/
- Frontmatter (verbatim):
  ```
  name: angular-architecture
  description: >-
    Modern Angular 20+ architecture with standalone components, signals, native template control
    flow, OnPush change detection, Scope Rule placement, and Screaming Architecture. Use when writing,
    editing, or reviewing Angular components, services, templates, routes, feature folders, shared
    code placement, or Angular architectural decisions.
  license: MIT
  metadata:
    version: "1.0.0"
  ```
- Bytes: 7577
- Has references/: N
- SHA-256 (local): `bf3930a0e82ee20d72e384bde1bb227594617c558d0b81ea45b7ed9e89c3dbfe`
- Content excerpt (first 30 lines, no frontmatter):
  > # Angular Architecture
  > Build Angular apps where the folder structure tells the business story and component placement is
  > based on real scope, not habit...

## Upstream search
- Search performed:
  - `git -C /Users/acosta/Dev/dallay/agents-skills log --all --oneline -- skills/angular-architecture/` → **empty** (no commits reference this path).
  - `git -C /Users/acosta/Dev/dallay/agents-skills log --all --oneline --diff-filter=A -- "skills/angular-architecture/SKILL.md"` → **empty**.
  - `git status` confirms `skills/angular-architecture/` is **untracked** in the worktree.
  - `gh search repos "angular architecture standalone signals skill"` → no candidate upstream repo.
  - `gh search code "Standalone First" "Scope Rule"` → returned GitHub repositories like `awesome-angular`, `angular-best-practices`, etc., but none is a 1:1 match for the content of this SKILL.md; the closest analogues are Angular team docs (`angular.dev`) and community books. None can be pinned at an immutable commit matching this content.
- Candidates found:
  - **No authoritative upstream candidate.** The skill is untracked in `agents-skills` git history. The `git log` is empty for this path. There is no upstream repository where this exact content exists at an immutable commit.
- Match analysis: Content reads as original prose in Yuniel-original voice (Cuban-style examples, "Scope Rule" and "Screaming Architecture" framings). It is consistent with the persona in the orchestrator's `AGENTS.md` ("CONCEPTS OVER CODE", "AGAINST IMMEDIACY", "SOLID FOUNDATIONS"). No upstream commit can be pinned.

## License evidence
- License claimed in local frontmatter: MIT.
- License at upstream: **N/A** — no upstream candidate.
- Immutable commit pin: **N/A** — no upstream candidate. The local SHA-256 (`bf3930a0e82ee20d72e384bde1bb227594617c558d0b81ea45b7ed9e89c3dbfe`) is the only attestation available.
- Maintainer permission: The skill author is unstated in frontmatter (`metadata.author` field absent). Author likely Yuniel (orchestrator persona) per proposal §"agents-skills content"; this needs explicit user confirmation before committing as `dallay-original`.

## Verdict
- **DEFER** (authorship unconfirmed; can become ACCEPT-WITH-EVIDENCE as `dallay-original` once author confirmed).
- Reason: REQ-SKILLREC-008 allows `metadata.author = "dallay-team"`, `metadata.source = "dallay-original"` when no upstream exists. The frontmatter currently has `license: MIT` but no `metadata.author` or `metadata.source`. Once the user (Yuniel) confirms authorship, this skill can ship in PR 2 with the frontmatter patch below. Without that confirmation, accepting the skill would commit un-attributed original content.
- Frontmatter patch required (only if user confirms authorship):
  ```
  license: MIT
  metadata:
    author: dallay-team
    source: dallay-original
    version: "1.0.0"
  ```
  AND a `PROVENANCE.md` `Materialized entries` row + SHA-256 line for `angular-architecture/SKILL.md`.

## Open
- Explicit confirmation from Yuniel that this is original work (or a derivative of public Angular team documentation).
- If derivative: which upstream source / commit pin applies.
- If original: confirmation that MIT license claim is the intended licensing for the `dallay-team` contribution.
