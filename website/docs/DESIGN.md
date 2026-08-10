---
name: AgentSync Docs
description: Dark-first technical documentation system — neon cyan and data violet on deep terminal black, glass surfaces, precise micro-motion.
colors:
  neon-cyan: "#00e5c4"
  neon-cyan-hover: "#00d4b4"
  data-violet: "#7c4dff"
  data-violet-hover: "#6b3de8"
  terminal-black: "#07070a"
  elevated-black: "#0d0d12"
  surface-black: "#121218"
  card-glass: "rgba(18, 18, 24, 0.6)"
  text-white: "#ffffff"
  slate-gray: "#9aa7b3"
  muted-gray: "#858e9a"
  border-white-08: "rgba(255, 255, 255, 0.08)"
  border-white-15: "rgba(255, 255, 255, 0.15)"
  paper-white: "#f8fafc"
  elevated-white: "#ffffff"
  surface-slate: "#e2e8f0"
  ink-dark: "#0f172a"
  ink-slate: "#475569"
  ink-muted: "#64748b"
typography:
  display:
    fontFamily: "Geist Sans, system-ui, -apple-system, sans-serif"
    fontSize: "clamp(2rem, 5vw, 3.5rem)"
    fontWeight: 800
    lineHeight: 1.1
    letterSpacing: "-0.03em"
  headline:
    fontFamily: "Geist Sans, system-ui, -apple-system, sans-serif"
    fontSize: "clamp(1.5rem, 3vw, 2rem)"
    fontWeight: 700
    lineHeight: 1.2
  title:
    fontFamily: "Geist Sans, system-ui, -apple-system, sans-serif"
    fontSize: "1.1rem"
    fontWeight: 600
  body:
    fontFamily: "Geist Sans, system-ui, -apple-system, sans-serif"
    fontSize: "1rem"
    lineHeight: 1.6
  label:
    fontFamily: "Geist Sans, system-ui, -apple-system, sans-serif"
    fontSize: "0.95rem"
    fontWeight: 600
  mono:
    fontFamily: "Geist Mono, ui-monospace, Cascadia Code, monospace"
rounded:
  sm: "6px"
  md: "10px"
  lg: "16px"
  xl: "24px"
spacing:
  xs: "4px"
  sm: "8px"
  md: "16px"
  lg: "24px"
  xl: "32px"
  2xl: "48px"
  3xl: "64px"
components:
  button-primary:
    backgroundColor: "{colors.neon-cyan}"
    textColor: "{colors.terminal-black}"
    rounded: "{rounded.md}"
    padding: "14px 24px"
  button-primary-hover:
    backgroundColor: "{colors.neon-cyan}"
    textColor: "{colors.terminal-black}"
    rounded: "{rounded.md}"
    padding: "14px 24px"
  button-secondary:
    backgroundColor: "{colors.card-glass}"
    textColor: "{colors.text-white}"
    rounded: "{rounded.md}"
    padding: "14px 24px"
  feature-card:
    backgroundColor: "{colors.card-glass}"
    textColor: "{colors.text-white}"
    rounded: "{rounded.lg}"
    padding: "24px"
  tech-badge:
    backgroundColor: "{colors.card-glass}"
    textColor: "{colors.slate-gray}"
    rounded: "{rounded.sm}"
    padding: "8px 16px"
  hero-badge:
    backgroundColor: "rgba(0, 229, 196, 0.08)"
    textColor: "{colors.neon-cyan}"
    rounded: "999px"
    padding: "4px 16px"
---

# Design System: AgentSync Docs

## Overview

**Creative North Star: "El Terminal Luminoso"**

AgentSync Docs is a dark-first developer documentation site that reads like a well-tuned terminal session: deep near-black backgrounds, a single confident neon-cyan accent paired with data violet, and precise, restrained motion. The system treats the docs as tooling, not marketing — every visual decision serves scanability and trust. Light mode is a faithful paper mirror of the same system: the terminal becomes warm slate and white, the cyan accent yields to violet, and the hierarchy survives unchanged.

The personality is precise and alive: glass surfaces with soft blur sit on layered blacks; shadows appear on hover as a response to state, never at rest; gradient text is reserved for the product name and primary CTA — a signature, not decoration. Density is documentation-standard with generous breathing room: 4–64px spacing scale, max-width containers around 1100–1200px, and touch targets hardened to 44px for every interactive control.

**Key Characteristics:**
- Dark-first: layered blacks (#07070a → #121218) build depth without shadows at rest
- Dual-accent palette: neon cyan (primary) + data violet (secondary), used with rarity
- Glass surfaces: 12px backdrop blur + translucent cards, elevated by border-lightening on hover
- Micro-motion only: fade-in-up entrance, glow pulse, float — all killed under `prefers-reduced-motion`
- Developer voice: Geist Sans for UI, Geist Mono for code, tight tracking on display type
- WCAG 2.2 AA baseline: muted text at 5.85:1 on elevated black, 44px touch targets

## Colors

The palette is a terminal world: near-black neutrals that layer, one electric accent for action, and a violet secondary used where the cyan alone would overheat. All values live as `--as-*` custom properties in `custom.css`; never hardcode a hex.

### Primary
- **Neon Cyan** (#00e5c4): the single action accent. Used for links on hover (dark), focus outlines, the hero badge dot, the primary CTA gradient, and `--sl-color-accent`. Its rarity is its power.

### Secondary
- **Data Violet** (#7c4dff): the cool counterpart. Powers the primary gradient's destination, light-mode accent (`--sl-color-accent`), and the secondary hero glow. Never used alone for primary actions.

### Neutral
- **Terminal Black** (#07070a): base background. The void the UI sits on.
- **Elevated Black** (#0d0d12): header, sidebar, footer — surfaces one step above base.
- **Surface Black** (#121218): code blocks, search, hover fill for glass.
- **Card Glass** (rgba(18, 18, 24, 0.6)): translucent card fill behind 12px blur.
- **Text White** (#ffffff): primary text and headings on dark.
- **Slate Gray** (#9aa7b3): secondary text, muted labels on dark.
- **Muted Gray** (#858e9a): footnote/tagline text — 5.85:1 on elevated black (AA).
- **Border White 08 / 15** (rgba(255,255,255,0.08) / 0.15): resting border → hover border.

Light mode overrides (`[data-theme="light"]`): **Paper White** (#f8fafc) base, **Elevated White** (#ffffff) header/sidebar, **Surface Slate** (#e2e8f0) fills, **Ink Dark** (#0f172a) text, **Ink Slate** (#475569) secondary, **Ink Muted** (#64748b) muted. Hover links switch from cyan to violet in light.

### Named Rules
**The Terminal Rarity Rule.** Cyan touches ≤10% of any screen: one gradient CTA, the badge dot, focus rings. When cyan appears twice on a surface, one of them is wrong.

**The Layered-Black Rule.** Depth comes from stacking `--as-bg-base → --as-bg-elevated → --as-bg-surface`, not from shadowing the background. Surfaces are flat at rest; shadow is a hover response.

## Typography

**Display Font:** Geist Sans (system-ui, -apple-system, sans-serif fallbacks)
**Body Font:** Geist Sans (same stack)
**Label/Mono Font:** Geist Mono (ui-monospace, "Cascadia Code", monospace)

**Character:** A modern geometric sans (Geist) carrying tight, confident display type and relaxed body copy, paired with a clean mono for all code — the classic developer-tool voice: precise, legible, unadorned.

### Hierarchy
- **Display** (800, clamp(2rem, 5vw, 3.5rem), 1.1, -0.03em): hero title only. The one place type gets expressive.
- **Headline** (700, clamp(1.5rem, 3vw, 2rem), 1.2): section headings (`Features`, page H1s).
- **Title** (600, 1.1rem, 1.2): card titles, sidebar entries.
- **Body** (400, 1rem, 1.6): documentation prose. Hero sub keeps max-width 500px; body line length follows Starlight's readable measure (~65–75ch).
- **Label** (600, 0.95rem, normal): buttons, tabs, badges, footer links.
- **Mono** (400, 0.9em in code context): all commands, config keys, inline code. Inline code gets 2px 6px padding, 4px radius, surface fill, hairline border.

### Named Rules
**The Mono Contract.** Every command, file path, and flag is set in Geist Mono — never in sans. Code blocks: surface background, hairline border, `--as-shadow-sm` at rest. In light mode, code text is forced to readable ink (#1e293b blocks / #334155 inline).

**The Tight Display Rule.** Display and headline weights go 700–800 with negative tracking (-0.03em display). Body never drops below 1rem.

## Layout

The layout is a single-column documentation spine with two full-width feature moments: the 80vh hero and the feature grid. Hero inner uses a 2-column grid (1fr + 400px visual) collapsing to a centered single column at ≤900px (visual moves above copy). Feature grid: `repeat(auto-fit, minmax(280px, 1fr))` with 24px gap.

Containers: hero content max 1200px; features and footer inner max 1100px; margins 0 auto. Section rhythm: 64px (`--as-space-3xl`) above/below major blocks; 32px between sections and their headings; 24px card padding; 16px between stacked elements.

Spacing scale (4/8/16/24/32/48/64px — `--as-space-xs` → `--as-space-3xl`) is the only rhythm vocabulary. Responsive behavior is token-driven: padding steps down to 16px on the hero at ≤900px and the footer switches column→row at ≥640px.

## Elevation & Depth

Hybrid: **glass + layered shadow**. Rest state is flat by definition — depth is communicated by background layering (base → elevated → surface) and by translucent cards with 12px backdrop blur. Shadows enter only as a state response: cards lift `translateY(-4px)` with `--as-shadow-md` on hover; buttons lift `translateY(-2px) scale(1.02)` with a cyan-tinted glow; the hero robot sits on a `drop-shadow(0 16px 40px rgba(0,0,0,0.5))`.

### Shadow Vocabulary
- **Shadow Sm** (`0 2px 8px rgba(0,0,0,0.3)`): code blocks at rest.
- **Shadow Md** (`0 8px 24px rgba(0,0,0,0.4)`): card and feature-item hover.
- **Shadow Lg** (`0 16px 48px rgba(0,0,0,0.5)`): reserved for overlay-level emphasis.
- **Shadow Glow** (`0 0 40px rgba(0,229,196,0.25)`): pulse-glow animation on the primary CTA.
- **Glass Blur**: `backdrop-filter: blur(12px)` on header, cards, feature items.

Light mode shadows soften (0.06/0.08/0.12 alpha) and glows drop to 0.15/0.10 — the system still lifts, but on paper.

### Named Rules
**The Glass-At-Rest Rule.** Cards are translucent glass at rest. Hover = border lightens (08→15) + lift + shadow. Never shadow a resting surface; never blur a solid surface.

## Shapes

The form language is **rounded-but-technical**: gentle radii everywhere with two distinctive shapes — fully-pill badges and the glass card. Corner vocabulary: sm 6px (tech badges, scrollbar thumb 4px), md 10px (buttons, search, code blocks, theme select), lg 16px (cards, feature items), xl 24px (reserved, e.g. large containers), 999px pills (hero badge, badge dot).

Borders are hairline: `rgba(255,255,255,0.08)` at rest, `0.15` on hover. Focus is a 2px cyan outline with 2px offset (`:focus-visible`). The scrollbar is a slim 8px custom track (elevated black track, hover-border thumb → muted gray on hover).

## Components

### Buttons
- **Shape:** rounded-md (10px), padding 14px 24px, font-weight 600, 0.95rem.
- **Primary:** gradient `linear-gradient(135deg, neon-cyan → data-violet)`, text Terminal Black, `--as-shadow` cyan glow (0 8px 24px rgba(0,229,196,0.2)). Arrow icon slides +4px on hover.
- **Hover / Focus:** `translateY(-2px) scale(1.02)` + intensified glow (0 12px 32px rgba(0,229,196,0.3)); 2px cyan `:focus-visible` outline.
- **Secondary / Ghost:** translucent card glass fill, white text, hairline border; hover = surface fill + border 15, no lift.

### Tech Badges
- **Style:** rounded-sm (6px), card-glass fill, hairline border, Slate Gray text, 0.8rem, 6px cyan dot before label.
- **State:** hover = surface fill, white text, border 15. Wraps under the hero CTA.

### Cards / Containers
- **Corner Style:** rounded-lg (16px).
- **Background:** card glass `rgba(18,18,24,0.6)` + 12px backdrop blur.
- **Shadow Strategy:** flat at rest; `--as-shadow-md` + `translateY(-4px)` on hover (see Elevation).
- **Border:** hairline 08 → 15 on hover.
- **Internal Padding:** 24px (`--as-space-lg`); icon 2rem above title; title 600 1.1rem; description 0.9rem Slate Gray.

### Navigation
- **Header:** Elevated Black, 12px blur, hairline bottom border. Site title set in gradient text (cyan→violet, weight 700). Search button: Surface Black fill, hairline border, rounded-md, **min-height 44px** (2.75rem). Theme select and social icons: Slate Gray, hover cyan, theme select also **min-height 44px**.
- **Sidebar:** Elevated Black fill; current-page link keeps inverted text on hover/focus.
- **Tabs** (CommandTabs): Starlight tabs with **min-height 44px** touch targets (broad `[role="tablist"] [role="tab"]` rule).
- **Footer:** Elevated Black, hairline top border, centered column (row at ≥640px). Logo in gradient text; links Slate Gray → cyan hover; tagline in Muted Gray 0.875rem.

### Hero (Signature Component)
- **Structure:** 80vh section, `--as-hero-bg` vertical gradient (dark) / 135° paper gradient (light), twin radial glows (cyan 20%/top, violet 80%/bottom), 60px grid pattern at 0.015 alpha.
- **Badge:** pill (999px), cyan-tint fill, cyan text, pulsing 6px dot (2s ease-in-out infinite).
- **Visual:** 380px robot on a blurred cyan glow (300px, blur 40px, 4s glow-pulse), robot floats ±12px on a 6s cycle with a deep drop-shadow.
- **Entrance:** copy + visual `fadeInUp` 0.8s ease forwards, staggered 0.15s. **CRITICAL:** the class starts `opacity: 0` — under `prefers-reduced-motion` it must be restored to `opacity: 1` or the hero disappears.

## Do's and Don'ts

### Do:
- **Do** build every surface from the `--as-*` token scale (`custom.css`); never hardcode a hex in a component.
- **Do** keep all 44px touch-target rules (`site-search button`, `starlight-theme-select select`, `[role="tablist"] [role="tab"]`) **unlayered** — unlayered author styles beat every Starlight `@layer` regardless of specificity.
- **Do** preserve the reduced-motion block with its `opacity: 1` restore on `.animate-fade-in-up` elements — killing the animation without the restore hides the hero.
- **Do** use the global `.gradient-text` utility for gradient type; never re-declare it scoped in a component (it was duplicated in Footer and removed).
- **Do** keep `--as-text-muted` at AA: #858e9a on dark (5.85:1), #64748b on light (4.76:1).
- **Do** use `:focus-visible` (2px cyan outline, 2px offset) for keyboard focus everywhere.
- **Do** let hover color go cyan in dark mode and violet in light mode (`[data-theme="light"] a:hover`).

### Don't:
- **Don't** add shadows to resting surfaces — the system is flat at rest, depth is layering + hover.
- **Don't** use `!important` to beat Starlight; raise specificity (unlayered rules, or (0,3,0) scoped selectors) instead.
- **Don't** invent new color roles beyond the `--as-*` palette; if it needs a new value, add a token.
- **Don't** put display type (clamp up to 3.5rem) outside the hero title — it's the signature moment.
- **Don't** set body text below 1rem or in Geist Mono (mono is for code only).
- **Don't** introduce new entrance animations without adding their selectors to the reduced-motion block.
