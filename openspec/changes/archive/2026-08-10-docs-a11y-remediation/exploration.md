# Exploration: A11y Remediation — AgentSync Docs Site

Date: 2026-08-10
Change name: `docs-a11y-remediation` (recommended)
Scope: `website/docs/` (Astro 5 + Starlight 0.41.6, base path `/agentsync`)

## Current State

The docs site applies a custom design system (`website/docs/src/styles/custom.css`) on top of
Starlight via `customCss` + component overrides (`Hero`, `Footer` registered in
`astro.config.mjs` under `starlight.components`). All custom CSS is **unlayered**, while Starlight
wraps its styles in `@layer starlight.base|reset|core|content|components|utils` (declared in
`node_modules/@astrojs/starlight/style/layers.css`). Consequence: unlayered author CSS wins over
every Starlight layer regardless of specificity — all fixes below can be done **without
`!important`**, provided new rules are NOT wrapped in `@layer`.

## Audit Confirmation (verified against source + computed values)

### P1-1 — `prefers-reduced-motion` incomplete — CONFIRMED

`custom.css:325-329` only sets `scroll-behavior: auto`. `scroll-behavior: smooth` comes solely
from `custom.css:167-169` (Starlight does not set it). Still active under reduced motion:
- `Hero.astro:204` `.badge-dot` → `pulse` (2s, infinite)
- `Hero.astro:354` `.robot-glow` → `glow-pulse` (4s, infinite)
- `Hero.astro:376` `.robot` → `float` (6s, infinite)
- `Hero.astro:392` `.animate-fade-in-up` → `fadeInUp` (0.8s, forwards)

**CRITICAL related defect (not in the audit, must be fixed together):**
`Hero.astro:390-393` `.animate-fade-in-up { opacity: 0; animation: fadeInUp 0.8s ease forwards; }`
sets the start state to `opacity: 0`. Any reduced-motion block that kills the animation WITHOUT
setting `opacity: 1` leaves the hero copy and hero visual **invisible**. The block MUST include
`.animate-fade-in-up { opacity: 1; }` (or equivalent).

Secondary findings:
- `custom.css:288` `@keyframes fadeInUp` (translateY 20px, 0.6s) + `:311` `.animate-fade-in-up`
  duplicate the Hero.astro copy; the scoped Hero rule wins on hero elements.
- `custom.css:300-317` `@keyframes pulse-glow` + `.animate-pulse-glow` (3s infinite) — **dead
  code**, used nowhere in the repo.
- Hover transforms (brief, user-initiated): `FeatureItem.astro:41` `translateY(-4px)`,
  `Hero.astro:268` btn `translateY(-2px) scale(1.02)`, `Hero.astro:277` `.btn-arrow translateX(4px)`,
  `custom.css:234` `.card:hover translateY(-2px)`, `Hero.astro:309` `.tech-badge transition: all`.
  Decision point: include or not in the reduce block (recommend: kill infinite/entrance
  animations; leave hover transforms).
- Starlight's own reduced-motion: only guards a `<details>` marker transition
  (`style/markdown.css:231` under `prefers-reduced-motion: no-preference`). No global kill; our
  block is the right home for the fix.

### P1-2 — Footer muted contrast (dark) — CONFIRMED (computed)

`--as-text-muted` dark `#6b7280` on `--as-bg-elevated` `#0d0d12` = **4.01:1** (fails AA 4.5).
Light `#64748b` on `#ffffff` = **4.76:1** (passes). Used only in:
- `Footer.astro:77` `.as-footer-tagline` (0.875rem) and `:108` `.as-footer-copy` (0.8rem)
- `custom.css:198` `::-webkit-scrollbar-thumb:hover` (non-text — cosmetic, no WCAG requirement)

Tokens that PASS (do not touch): `--as-text-secondary` dark `#9aa7b3` = 7.89:1 on `#0d0d12`;
light `#475569` = 7.58:1 on white.

Candidate dark fix values (on `#0d0d12`): `#7d8590` → 5.20:1 · `#858e9a` → 5.85:1 ·
`#8b949e` → 6.30:1. Recommend token-level update (affects footer + scrollbar hover — both
improve); alternatively override only within Footer.

### P2-3 — `.gradient-text` duplicated — CONFIRMED

Identical implementation at `custom.css:214-219` (uses `--as-gradient-primary` var) and
`Footer.astro:126-131` (hardcoded colors). Footer's scoped copy wins on the logo only due to
scoped attribute specificity; visual output is identical. Safe to delete the Footer copy — the
global unlayered utility applies the same result.

### P2-4 — Touch targets — CONFIRMED, all CSS-fixable

- **Search button**: `site-search > button[data-open-modal]` (`Search.astro`), `height: 2.5rem`
  (40px) at mobile, 34px wide. Override: `min-height: 2.75rem` on the existing `site-search
  button` rule in custom.css. CSS only.
- **Theme toggle**: `starlight-theme-select > label > select` (`Select.astro`, 40px). Override:
  `min-height: 2.75rem` on the existing `starlight-theme-select select` rule. CSS only.
- **Tabs**: `CommandTabs.astro` renders Starlight `<Tabs syncKey="pkg">` →
  `starlight-tabs > .tablist-wrapper > ul[role="tablist"] > li.tab > a[role="tab"]`
  (`user-components/Tabs.astro`), `padding: 0.275rem 1.25rem` + `line-height:
  var(--sl-line-height-headings)` ≈ 28px. Override `[role="tablist"] [role="tab"] { min-height:
  2.75rem; }` in custom.css. CSS only. Blast radius: 58 CommandTabs instances across 6 pages
  (`reference/cli.mdx` alone: 18 → 72 tab buttons); `.tablist-wrapper` already has
  `overflow-x: auto`, so taller tabs scroll instead of wrapping on mobile.
- **Heading anchor links**: `.sl-anchor-link` (visible ≈20x29) BUT already expands its hit area via
  `::after { inset: -0.25rem -0.5rem }` (`style/anchor-links.css`). Effective target ≈ 36-40px
  wide × ~35px tall — likely already ≥ WCAG 2.2 AA 24x24. Reaching the audit's 44px bar needs a
  bigger `::after` inset; measure in-browser during apply before deciding.

**Scope nuance to surface to the user**: the audit used 44px (Apple HIG / WCAG AAA). WCAG 2.2 AA
(SC 2.5.8) requires only 24x24 CSS px. The change should state which standard it targets; only the
search/theme/tabs targets clearly fail the 44px bar, and anchors may already pass AA.

## Affected Areas

| File | Change |
|------|--------|
| `website/docs/src/styles/custom.css` | Extended `prefers-reduced-motion` block (kill hero animations + restore `opacity: 1`); dark `--as-text-muted` bump (token-level); min-height rules for `site-search button`, `starlight-theme-select select`, `[role="tablist"] [role="tab"]`; optional `.sl-anchor-link::after` inset bump; optional dead-code cleanup of `.animate-pulse-glow` |
| `website/docs/src/components/Hero.astro` | Preferred: no markup change — handle purely in the reduce block in custom.css. Alternative: add a class/data-attr hook for animations. |
| `website/docs/src/components/Footer.astro` | Delete duplicated scoped `.gradient-text` rule |
| `website/docs/src/components/FeatureItem.astro` | Only if hover transforms are included in the reduce block — otherwise untouched |

Not touched: `CommandTabs.astro` (renders Starlight Tabs; fix is CSS), Starlight internals,
`astro.config.mjs`.

## Approaches

1. **CSS-only remediation (recommended)** — all fixes in `custom.css` (+ small Footer cleanup).
   Reduced-motion via a scoped `*`-kill for infinite/entrance animations plus explicit
   `opacity: 1` restore; token bump for muted; min-height rules for the three control types.
   - Pros: smallest diff (1 file + 1 small cleanup), no component rewrites, no `!important`
     needed thanks to unlayered-vs-layered cascade, trivially reversible.
   - Cons: touches Starlight internal selectors (`[role="tablist"] [role="tab"]`,
     `button[data-open-modal]`) — keep selectors broad to survive Starlight upgrades.
   - Effort: Low.

2. **Component-level fixes** — rewrite `CommandTabs.astro` with custom tab markup/classes, patch
   `Hero.astro` with explicit animation hooks.
   - Pros: no dependence on Starlight internals; more explicit.
   - Cons: larger diff, more review surface, duplicates Starlight behavior (sync/restore logic
     lives in the Tabs web component), higher maintenance.
   - Effort: Medium.

3. **Hybrid** — CSS for contrast/touch-targets; add a `data-reduced-motion` hook only where CSS
   can't express the intent (nowhere strictly needed here).
   - Effort: Low.

## Recommendation

Approach 1 (CSS-only). All four audit findings are addressable in `custom.css` with two small
component cleanups (Footer `.gradient-text`; optionally Hero/FeatureItem untouched). The critical
must-do is the `opacity: 1` restore for `.animate-fade-in-up` in the reduce block. Choose a dark
muted candidate ≥ 4.5:1 (e.g. `#858e9a`, 5.85:1). Target the 44px bar for search/theme/tabs;
re-measure anchor links in-browser (they may already pass WCAG 2.2 AA via the `::after` expansion).

## Risks

- **Invisible hero under reduced motion** if the block kills `fadeInUp` without restoring
  `opacity: 1` — the #1 correctness risk.
- **Starlight internal selectors**: `[role="tablist"] [role="tab"]` and `button[data-open-modal]`
  are package internals; a Starlight major upgrade could rename them. Keep selectors broad and
  re-verify after upgrades. Current version pinned: 0.41.6.
- **Cascade trap**: new rules MUST stay unlayered (no `@layer` wrapper) to beat Starlight layers;
  if someone wraps them later, the fixes silently stop applying.
- **Token bump side effects**: changing dark `--as-text-muted` also changes the
  `scrollbar-thumb:hover` color (cosmetic improvement, but a visible diff in the scrollbar).
- **Tabs height growth**: taller tabs (~44px) slightly enlarge code-block sections on all pages
  using CommandTabs (58 instances) — expected, but flag in the proposal for visual review.
- **Theme asymmetry**: contrast fix is dark-only by design (light already passes); reduced-motion
  and touch-target rules are theme-agnostic; `.gradient-text` removal is theme-agnostic.
- **Anchor `::after` inset increase** could overlap adjacent heading text if enlarged too much —
  keep `inset` tweaks conservative.

## Ready for Proposal

Yes. All four audit findings verified against source with computed contrast values; mechanisms
identified; fix approach chosen (CSS-only). Orchestrator should tell the user: findings confirmed
(plus one CRITICAL hidden defect: invisible hero under reduced motion), the fix is ~1 CSS file +
1 small component cleanup, and ask whether the touch-target bar is 44px (audit standard) or
24px (WCAG 2.2 AA), since that determines whether heading anchor links need changes.
