# Evidence — typescript-strict-patterns

## Local state
- Path: /Users/acosta/Dev/dallay/agents-skills/skills/typescript-strict-patterns/
- Frontmatter (verbatim):
  ```
  name: typescript-strict-patterns
  description: >-
    Strict TypeScript patterns for safer application code: const-derived union types, flat
    interfaces, `unknown` narrowing, type guards, utility types, generics, and type-only imports. Use
    when writing, editing, or reviewing TypeScript code involving types, interfaces, DTOs, API
    payloads, configuration objects, or avoiding `any`.
  license: MIT
  metadata:
    version: "1.0.0"
  ```
- Bytes: 5273
- Has references/: N
- SHA-256 (local): `edc0508110aa7e6b58904d84eea03bcd22cb41e6feb2717202ce663154e91e84`
- Content excerpt (first 30 lines, no frontmatter):
  > # TypeScript Strict Patterns
  > Use TypeScript to encode real domain boundaries and make refactors safe...

## Upstream search
- Search performed:
  - `git -C /Users/acosta/Dev/dallay/agents-skills log --all --oneline -- skills/typescript-strict-patterns/` → **empty** (no commits reference this path).
  - `git -C /Users/acosta/Dev/dallay/agents-skills log --all --oneline --diff-filter=A -- "skills/typescript-strict-patterns/SKILL.md"` → **empty**.
  - `git status` confirms `skills/typescript-strict-patterns/` is **untracked** in the worktree.
  - `gh search repos "typescript strict patterns skill"` → no candidate upstream repo.
  - `gh search code "Const-Derived Union Types"` → matches many TypeScript-style guides (Total TypeScript, TypeScript handbook, etc.) but none is a 1:1 byte-match. The content here reads as original composition (specific examples, "Common Mistakes" pattern matching the angular-architecture style).
- Candidates found:
  - **No authoritative upstream candidate.** Untracked in `agents-skills` git history. No upstream repository with byte-identical content at an immutable commit.
- Match analysis: Content reads as original prose in Yuniel-original voice. Consistent with the orchestrator's `AGENTS.md` ("TypeScript guidance: strongly typed (avoid any unless strictly justified)") and with the Angular-architecture style (parallel section structure, similar checklist / common mistakes pattern).

## License evidence
- License claimed in local frontmatter: MIT.
- License at upstream: **N/A** — no upstream candidate.
- Immutable commit pin: **N/A** — no upstream candidate. The local SHA-256 (`edc0508110aa7e6b58904d84eea03bcd22cb41e6feb2717202ce663154e91e84`) is the only attestation available.
- Maintainer permission: The skill author is unstated in frontmatter (`metadata.author` field absent). Author likely Yuniel per the orchestrator persona; needs explicit user confirmation before committing as `dallay-original`.

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
  AND a `PROVENANCE.md` `Materialized entries` row + SHA-256 line for `typescript-strict-patterns/SKILL.md`.

## Open
- Explicit confirmation from Yuniel that this is original work (or a derivative of a public TypeScript pattern guide).
- If derivative: which upstream source / commit pin applies.
- If original: confirmation that MIT license claim is the intended licensing for the `dallay-team` contribution.
