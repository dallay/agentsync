# Landing polish — docs splash (`website/docs/src/content/docs/index.mdx`)

Route: Direct inline (in-thread polish; skill subagents unavailable in this harness).
Scope: refinement only. Preserve El Terminal Luminoso (DESIGN.md), content, behavior.

## Triage (polish.md order)

1. Broken paths (P1): MDX body uses raw `<a href="/guides/...">` which ignores `base: /agentsync`.
   Props (`FeatureItem href`, hero actions, footer) already go through `withBase`. Fix body links only.
2. Brand drift (P3): `Why agentsync?` → `Why AgentSync?` per PRODUCT Brand Commitments.
3. A11y/CLS (P2/P4): hero `alt="agentsync robot"` generic, no width/height → CLS risk;
   `FeatureItem` icon emoji exposed to AT. Add `aria-hidden`, descriptive alt, explicit dimensions.
4. Stale defaults: `Hero.astro` fallback title/tagline differ from frontmatter. Align fallbacks.
5. Out of scope (not smuggling redesign): emoji → drawn SVG icon system; tech-badge coverage
   (VS Code, Z-Code, MiniMax, Claude Desktop); new sections. Recommend as follow-ups.

## Tasks

- [x] Establish system (DESIGN.md, custom.css, Hero/Features/FeatureItem/CommandTabs/Footer, live fetch)
- [ ] Fix base-aware body links via `BaseLink` in `index.mdx`
- [ ] Brand capitalization + hero fallback alignment + img robustness in `Hero.astro`
- [ ] `aria-hidden` on `FeatureItem` icon
- [ ] Build docs (`pnpm run docs:build`) + detector pass on changed targets
- [ ] Batched desktop+mobile inspection, one fix batch, max one confirm round

## Evidence

- RESOLVED_CONTEXT platform `web`, product `PRODUCT.md`, design `DESIGN.md`
- Live: https://dallay.github.io/agentsync/ renders hero + 7 cards + Quick Start
- Prior critique: none (`critique-storage.mjs` exit 2)
- Detector (manual required, no auto hook): run once at end, not during concept

## Status

Ready — edits applied, `docs:build` passes (17 pages), built HTML verified,
detector run once (9 pre-existing advisories, 0 new). No visual-layout changes.
