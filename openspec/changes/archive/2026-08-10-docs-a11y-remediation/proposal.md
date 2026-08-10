# Proposal: Docs Site A11y Remediation (WCAG 2.2 AA)

## Intent

The AgentSync docs site (`website/docs/`, Astro 5 + Starlight 0.41.6) fails an a11y audit: `prefers-reduced-motion` handling is incomplete and can leave the hero **invisible**; footer dark muted text is 4.01:1 (below AA 4.5:1); search/theme/tabs controls have sub-44px touch targets; `.gradient-text` is duplicated. All findings are CSS-fixable.

## Scope

### In Scope
- Extended `prefers-reduced-motion` block (`custom.css:325-329`): kill hero animations (`pulse`, `glow-pulse`, `float`, `fadeInUp`) **and** restore `opacity: 1` on `.animate-fade-in-up` (critical — starts at `opacity: 0`, else hero invisible). Hover transforms stay (user-initiated).
- Dark-only `--as-text-muted` bump: `#6b7280` → `#858e9a` (5.85:1, ≥1.3x AA margin). Light theme untouched (4.76:1 passes).
- Touch targets at **44px** (`min-height: 2.75rem`) for `site-search button`, `starlight-theme-select select`, `[role="tablist"] [role="tab"]`.
- Delete scoped `.gradient-text` in `Footer.astro:126-131` (global utility `custom.css:214-219` applies identically).

### Out of Scope
- `.sl-anchor-link` — already passes WCAG 2.2 AA (SC 2.5.8) via `::after` inset; not touched.
- Component rewrites (`CommandTabs.astro`, `Hero.astro`), Starlight internals, `astro.config.mjs`.
- Light-theme tokens; hover transforms under reduced motion; dead `.animate-pulse-glow` cleanup (deferred).

## Capabilities

### New Capabilities
- `docs-site-a11y`: docs site MUST keep hero visible under `prefers-reduced-motion`; footer dark muted text MUST meet AA ≥4.5:1; search/theme/tab controls MUST expose ≥44px targets; `.gradient-text` MUST come only from the global utility.

### Modified Capabilities
- None. The `documentation` spec governs content, not the visual layer.

## Approach

CSS-only (exploration Approach 1). Rules added **unlayered** in `website/docs/src/styles/custom.css` — unlayered author CSS beats every Starlight `@layer` without `!important`. Reduce block kills the four hero animations + sets `.animate-fade-in-up { opacity: 1 }`; dark muted token bumped at token level (also improves `scrollbar-thumb:hover`, cosmetic); three `min-height` rules. Footer cleanup removes the duplicate.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `website/docs/src/styles/custom.css` | Modified | Reduce block, dark `--as-text-muted`, 3 min-height rules |
| `website/docs/src/components/Footer.astro` | Modified | Remove scoped `.gradient-text` |
| `openspec/specs/docs-site-a11y/spec.md` | New | Capability spec (sdd-spec) |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Invisible hero if opacity restore missed | High | Explicit `opacity: 1` in reduce block + verify under emulation |
| Starlight internal selectors break on upgrade | Med | Keep selectors broad; re-verify (pinned 0.41.6) |
| Rules wrapped in `@layer` silently stop applying | Med | Keep unlayered; CSS comment warning |
| Tabs height growth (58 CommandTabs instances) | Med | Expected; `.tablist-wrapper` already `overflow-x: auto` |

## Rollback Plan

Revert the single commit: all changes live in 2 files (`custom.css` + `Footer.astro`); no markup, config, or data migration. `git revert` suffices.

## Dependencies

- Starlight 0.41.6 (pinned) internal selectors; no new packages.

## Success Criteria

- [ ] Under reduced-motion emulation: hero visible (`opacity: 1`), no infinite/entrance animations
- [ ] Footer dark muted contrast ≥ 4.5:1 (computed, e.g. 5.85:1)
- [ ] Search/theme/tab hit areas ≥ 44px
- [ ] No `.gradient-text` duplication; `pnpm run docs:build` passes
