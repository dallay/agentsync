# Docs Site A11y Specification

## Purpose

Define the accessibility remediation contract for the AgentSync docs site visual layer
(`website/docs/`, Astro 5 + Starlight 0.41.6): reduced-motion safety that keeps the hero visible,
AA contrast for footer muted text in dark theme, 44px touch targets on interactive controls, a
single source for the `.gradient-text` utility, and no regressions to the docs build or desktop
layout. All changes are CSS-only in `website/docs/src/styles/custom.css` plus the removal of the
duplicated utility in `Footer.astro`.

## Requirements

### Requirement: Hero Remains Visible And Static Under Reduced Motion

When `prefers-reduced-motion: reduce` is active, the hero MUST disable the four entrance or
infinite animations (`pulse`, `glow-pulse`, `float`, `fadeInUp`). Because `.animate-fade-in-up`
starts at `opacity: 0`, the hero copy and hero visual MUST be restored to `opacity: 1` in the same
block; otherwise the hero becomes invisible. The block SHALL NOT disable user-initiated hover
transforms.

#### Scenario: Reduced-motion user sees a visible, static hero

- GIVEN `prefers-reduced-motion: reduce` is active on the docs home page
- WHEN the hero renders
- THEN the badge-dot, robot-glow, robot, and fade-in-up animations MUST NOT run
- AND the hero copy and hero visual MUST render at `opacity: 1`

#### Scenario: Hover transforms remain available

- GIVEN `prefers-reduced-motion: reduce` is active
- WHEN the user hovers an interactive hero or feature element
- THEN the brief user-initiated transform SHALL still apply

### Requirement: Footer Muted Text Meets AA Contrast In Dark Theme

Footer tagline and copy text MUST have a contrast ratio of at least 4.5:1 against the dark elevated
background. The dark `--as-text-muted` token MUST be `#858e9a` (computed 5.85:1). The light-theme
token SHALL remain unchanged (`#64748b`, 4.76:1, already passing).

#### Scenario: Footer dark text passes AA

- GIVEN the docs site renders in dark theme
- WHEN the footer tagline and copy are painted with `--as-text-muted`
- THEN their contrast ratio on `--as-bg-elevated` MUST be ≥ 4.5:1
- AND the token value MUST be `#858e9a`

#### Scenario: Light theme stays untouched

- GIVEN the docs site renders in light theme
- WHEN the footer is painted
- THEN `--as-text-muted` MUST remain `#64748b`

### Requirement: Single Source For The Gradient Text Utility

The `.gradient-text` utility MUST be defined exactly once, in `custom.css`. The scoped copy in
`Footer.astro` MUST be removed. The footer logo SHALL keep its gradient appearance via the global
utility.

#### Scenario: No duplicated definitions

- GIVEN the docs site stylesheets
- WHEN searching for `.gradient-text` definitions
- THEN exactly one definition MUST exist in `custom.css`
- AND no definition MAY remain in `Footer.astro`

#### Scenario: Footer logo keeps its gradient

- GIVEN the footer logo carries the `gradient-text` class
- WHEN the site renders
- THEN the logo MUST render the gradient from the single global utility

### Requirement: Interactive Controls Expose 44px Touch Targets

On mobile, the search button (`site-search button`), the theme selector
(`starlight-theme-select select`), and every tab (`[role="tablist"] [role="tab"]`) MUST have a
minimum height of `2.75rem` (44px at 16px root). Heading anchor links SHALL remain unchanged —
they already meet WCAG 2.2 AA through their expanded `::after` hit area.

#### Scenario: Mobile controls meet the 44px bar

- GIVEN a mobile viewport on a docs page
- WHEN the search button, theme selector, or a tab is measured
- THEN its rendered height MUST be ≥ 44px

#### Scenario: Anchor links keep their hit area

- GIVEN a heading with an anchor link
- WHEN the site renders
- THEN the anchor link MUST receive no new sizing rule
- AND it MUST keep the existing expanded `::after` hit area

### Requirement: Build And Desktop Layout Do Not Regress

`pnpm astro build` in `website/docs` SHALL pass after the remediation. The desktop layout MUST NOT
break; the ~16px height growth of tab bars on pages using CommandTabs is acceptable and expected
(horizontal scroll is preserved by the existing `.tablist-wrapper` overflow).

#### Scenario: Docs build passes

- GIVEN the remediated styles and footer component
- WHEN `pnpm astro build` runs in `website/docs`
- THEN the build SHALL complete successfully

#### Scenario: Desktop layout tolerates taller tabs

- GIVEN a desktop viewport on a page using CommandTabs
- WHEN the page renders
- THEN the tabs MUST remain clickable and the layout MUST NOT break
- AND the taller tab bar MAY grow the section height without breaking overflow handling
