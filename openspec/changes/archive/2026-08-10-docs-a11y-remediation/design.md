# Design: Docs Site A11y Remediation (WCAG 2.2 AA)

## Context & Constraints

Docs site = Astro 5 + Starlight **0.41.6 (pinned)** + `website/docs/src/styles/custom.css`
(customCss) + component overrides (`Hero`, `Footer`). Four audit findings, all CSS-fixable.
No new packages; no config/markup changes.

**Cascade mechanism (exploited):** Starlight wraps styles in `@layer starlight.*`. Our
`custom.css` is **unlayered**, and unlayered author styles beat every layer *regardless of
specificity* — all fixes win **without `!important`**, provided new rules stay out of `@layer`.

**Exception — Hero scoped styles:** Astro injects `Hero.astro` scoped styles as unlayered
`<style>` AFTER custom.css; those rules (`[data-astro-cid-*]`, specificity 0,2,0) win tie-breaks
by source order. Reduced-motion selectors need specificity **≥ (0,3,0)**.

## Architecture Decisions

| # | Option | Tradeoffs | Decision |
|---|--------|-----------|----------|
| D1 | CSS-only vs component rewrites vs hybrid | rewrites = larger diff, duplicates Starlight Tabs sync logic; hybrid = needs a hook nothing requires | **CSS-only** — smallest diff, reversible, no `!important` |
| D2 | Dark muted: `#7d8590` 5.20:1 / `#858e9a` 5.85:1 / `#8b949e` 6.30:1 | 5.20:1 minimal margin; 6.30:1 visibly lighter token | **`#858e9a` (5.85:1)** — ≥1.3× AA margin, stays muted. Light `#64748b` untouched |
| D3 | Touch bar: 44px (HIG/AAA) vs 24px (WCAG 2.2 AA SC 2.5.8) | 24px = anchors pass today; 44px = stricter, catches the 3 controls | **44px** for search/theme/tabs; **anchors out of scope** (already ≥ AA via `::after` inset) |
| D4 | Reduce scope: infinite/entrance only vs also hover transforms | hover transforms are brief + user-initiated (WCAG-exempt) | **Kill infinite/entrance only**; hover stays |
| D5 | Muted bump: token-level vs Footer-only | token also recolors `scrollbar-thumb:hover` (cosmetic +) | **Token-level** (`custom.css:22`) |
| D6 | Theme `min-height`: add to compound rule `starlight-theme-select select, .social-icons a` vs split | compound rule grows `.social-icons a` to 44px (unwanted) | **Split** — own rule, color rule stays compound |

## Reduced-Motion Block (proposed design)

Replace `custom.css:325-329`:

```css
@media (prefers-reduced-motion: reduce) {
	html {
		scroll-behavior: auto;
	}

	/* Scoped Hero rules ([data-astro-cid], 0,2,0) inject after custom.css —
	   these selectors use (0,3,0) to win the unlayered tie-break. */
	.hero .hero-badge .badge-dot,
	.hero .robot-wrap .robot-glow,
	.hero .robot-wrap .robot {
		animation: none;
	}

	/* CRITICAL: .animate-fade-in-up starts at opacity: 0 (Hero.astro:390);
	   killing fadeInUp without restoring opacity hides the hero. */
	.hero .hero-copy.animate-fade-in-up,
	.hero .hero-visual.animate-fade-in-up {
		animation: none;
		opacity: 1;
	}
}
```

Kills `pulse`, `glow-pulse`, `float`, `fadeInUp` (hero-copy + hero-visual, incl. inline
`animation-delay: 0.15s`).

## Touch Target Rules (44px = 2.75rem @ 16px root)

| Control | Selector | Rule |
|---------|----------|------|
| Search | `site-search button` (exists `custom.css:148-152`) | add `min-height: 2.75rem;` to existing rule |
| Theme | `starlight-theme-select select` (exists `custom.css:155-159`) | **new** own rule `min-height: 2.75rem;` (D6) |
| Tabs | `[role="tablist"] [role="tab"]` | **new** `min-height: 2.75rem;` (broad, survives upgrades) |

Tab growth (~28→44px) is absorbed by `.tablist-wrapper`'s `overflow-x: auto`.

## Cascade Flow

```
custom.css (unlayered)                 Starlight layers           Hero scoped styles
    min-height: 2.75rem ──────>  beats @layer starlight.*
    --as-text-muted: #858e9a ──>  re-computed at footer use-site
    reduce block (0,3,0) ────────────────────────────────────> beats [data-astro-cid] (0,2,0)
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `website/docs/src/styles/custom.css` | Modify | Reduce block (D4), dark `--as-text-muted` (D2, :22), `min-height` on `site-search button`, new theme-select + tablist rules (D6) |
| `website/docs/src/components/Footer.astro` | Modify | Delete scoped `.gradient-text` (:126-131); global utility (`custom.css:214-219`) applies identically to logo `as-footer-logo gradient-text` |

## Known Side Effects

- **Scrollbar**: `::-webkit-scrollbar-thumb:hover` (:198) uses the token → recolored, cosmetic.
- **Tabs growth**: 58 CommandTabs instances / 6 pages (`reference/cli.mdx` alone: 18); code blocks grow ~16px. Expected; horizontal scroll preserved.
- **Theme asymmetry**: contrast fix dark-only by design; reduce/touch rules theme-agnostic.

## Verification Strategy

| Requirement | How to verify |
|-------------|---------------|
| Hero visible, no animation under reduced motion | DevTools emulate `prefers-reduced-motion: reduce` → screenshot: `opacity: 1`, no keyframe animations running |
| Dark muted ≥ 4.5:1 | Computed 5.85:1 + DevTools contrast checker on footer tagline/copy (dark) |
| Touch targets ≥ 44px | `getBoundingClientRect().height` on search button, theme select, tabs in `reference/cli.mdx` (68 tab buttons) |
| No `.gradient-text` duplication | grep → single definition; logo still renders gradient |
| Build | `pnpm run docs:build` passes |

## Migration / Rollout

No migration. Single reversible commit: runtime implementation changes in two files only (`custom.css` + `Footer.astro`). `git revert` removes the complete commit including runtime changes, OpenSpec specifications, and archive records. No flags.

## Open Questions

- [ ] None blocking — all decisions user-resolved. (Optional: re-measure anchor `::after` inset in-browser to document the AA pass.)
