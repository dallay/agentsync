# Tasks: Docs Site A11y Remediation (WCAG 2.2 AA)

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~55 (custom.css ≈45, Footer.astro −6) |
| 400-line budget risk | Low |
| Chained PRs recommended | No |
| Suggested split | Single PR |
| Delivery strategy | ask-on-risk (default) |
| Chain strategy | single-pr |

Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: single-pr
400-line budget risk: Low

### Suggested Work Units

| Unit | Goal | Likely PR | Notes |
|------|------|-----------|-------|
| 1 | Full remediation: reduce block + token bump + 3 touch rules + Footer cleanup | PR 1 | base `main`; 2 files, no migration, `git revert` rollback |

## Phase 1: Infrastructure (Baseline)

- [x] 1.1 Confirm anchors: custom.css:22 (dark muted), :148-152 (`site-search button`), :155-159 (compound theme-select), :214-219 (`.gradient-text`), :325-329 (reduce block); Footer.astro:126-131.
- [x] 1.2 Baseline: `pnpm run docs:build` green; hero screenshots (normal + reduced-motion) for before/after.

## Phase 2: Implementation

- [x] 2.1 Token: custom.css:22 dark `--as-text-muted` → `#858e9a`; light :97 `#64748b` untouched.
- [x] 2.2 Touch: add `min-height: 2.75rem;` to existing `site-search button` rule (custom.css:148-152).
- [x] 2.3 Touch: SPLIT `starlight-theme-select select` from compound :155-159 into own rule with `min-height: 2.75rem;` (`.social-icons a` stays compound, size unchanged).
- [x] 2.4 Touch: new broad unlayered rule `[role="tablist"] [role="tab"] { min-height: 2.75rem; }`.
- [x] 2.5 Reduced motion: replace :325-329 with design block — selectors (0,3,0): `.hero .hero-badge .badge-dot`, `.hero .robot-wrap .robot-glow`, `.hero .robot-wrap .robot` → `animation: none`; `.hero .hero-copy.animate-fade-in-up`, `.hero .hero-visual.animate-fade-in-up` → `animation: none; opacity: 1;` (CRITICAL — missing `opacity: 1` hides hero); keep `html { scroll-behavior: auto; }`; no hover-transform kills.
- [x] 2.6 Footer.astro: delete :126-131 scoped `.gradient-text`; global utility (custom.css:214-219) keeps logo gradient.
- [x] 2.7 CSS hygiene: comment in custom.css warning new rules MUST stay unlayered (no `@layer`) to keep beating Starlight layers.

## Phase 3: Testing (empirical verification, documented)

- [x] 3.1 Reduced-motion emulation (DevTools/Playwright `prefers-reduced-motion: reduce` on home): screenshot proves hero copy + visual at `opacity: 1`; no keyframe animations running (pulse/glow-pulse/float/fadeInUp).
- [x] 3.2 Touch: in-browser `getBoundingClientRect().height >= 44` on search button, theme select, tabs in `reference/cli.mdx` (68 tab buttons).
- [x] 3.3 Contrast: DevTools checker on footer tagline/copy (dark) — ≥4.5:1 (5.85:1 expected); verify light theme still `#64748b`.
- [x] 3.4 Grep: exactly one `.gradient-text` definition (custom.css), none in Footer.astro; logo still renders gradient.
- [x] 3.5 Regression: `.social-icons a` height unchanged; hover transforms still apply under reduced motion; desktop tabs clickable (overflow-x preserved).
- [x] 3.6 Build: `pnpm run docs:build` passes; record results for verify-report.
