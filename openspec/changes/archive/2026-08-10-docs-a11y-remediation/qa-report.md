# QA Report: Docs A11y Remediation

## 1. Identity

| Field | Value |
|-------|-------|
| Change | `docs-a11y-remediation` |
| Mode | openspec |
| Phase | `qa` (acceptance — capability-driven, user/operator observable behavior) |
| Date | 2026-08-10 |
| Executor | sdd-qa (independent acceptance gate, after sdd-verify) |

## 2. Source Artifacts and Verification Handoff

Read in full before testing:

- `openspec/changes/docs-a11y-remediation/proposal.md`
- `openspec/changes/docs-a11y-remediation/spec.md` — 5 requirements, 11 scenarios (acceptance criteria)
- `openspec/changes/docs-a11y-remediation/design.md`
- `openspec/changes/docs-a11y-remediation/tasks.md`
- `openspec/changes/docs-a11y-remediation/verify-report.md` — technical conformance **PASS** (5/5 reqs, 11/11 scenarios, no CRITICAL/WARNING). Handoff notes used: token `#858e9a`, block custom.css:342-361, touch rules :153/:174/:178-179, build exit 0, `.sl-anchor-link` hit area, `.card:hover` not live-exercised (SUGGESTION), mobile viewport not emulated (SUGGESTION).
- `openspec/changes/docs-a11y-remediation/state.yaml`
- `openspec/config.yaml` — no QA-specific policy overrides.

**Handoff contract**: verify owns technical conformance (static + live vs spec). QA evaluates the same contract from the acceptance perspective: does the delivered behavior satisfy what the user was promised, in the real running site? QA does not re-derive verify's static analysis; it independently exercises observable behavior and re-runs the build for acceptance evidence.

## 3. Target, Environment, Permissions, Limitations

| Item | Detail |
|------|--------|
| Target | Docs site (Astro 5 + Starlight 0.41.6), base `/agentsync`, dev server `http://localhost:4321` (confirmed serving changed code) |
| Browser | Playwright MCP, headless Chromium (macOS host) |
| Tested pages | `/agentsync/` (home, custom Hero/Footer), `/agentsync/reference/cli/` (CommandTabs, 17 tablists) |
| Viewports | Desktop 1280×800, narrow 500×800, mobile 375×812 |
| Motion modes | `no-preference` (normal) and `prefers-reduced-motion: reduce` — both explicitly emulated via `page.emulateMedia` |
| Themes | dark (default, `starlight-theme=dark`) and light — both measured |
| Permissions | None required; no auth surface on docs site. Read-only QA: no source modified |
| Build | `pnpm run docs:build` re-run by QA (repo root) — independent acceptance evidence |

**Limitations**:
- Headless browser, not a physical device. Mobile checks are viewport emulation (375×812); pointer-based clicks, not touch gestures.
- The literal `.card:hover { transform: translateY(-2px) }` transform could not be observed on a real element — no `.card` element is rendered on any tested page. Mitigated by (a) live dump of every rule inside the reduce media query proving **zero** `transform`/`:hover` declarations, and (b) live hover interactivity (color/background transition) firing under reduce.
- No tab content currently overflows its `.tablist-wrapper` at 375/500/1280px, so horizontal tab scroll never needs to activate; the capability is present (computed `overflow-x: auto`) — verified not removed.

## 4. Capability Inventory

| Capability | Status | Rationale |
|------------|--------|-----------|
| Browser automation (Playwright MCP) | **Selected** | Primary observable-behavior harness; drives navigation, reload, click, hover, viewport |
| Reduced-motion emulation (`page.emulateMedia`) | **Selected** | Required by REQ 1; both `reduce` and `no-preference` exercised |
| Computed-style measurement (in-page JS) | **Selected** | Numeric evidence: opacity, animation-name/duration, min-height, overflow, background-clip |
| WCAG contrast computation (in-page JS, WCAG 2.x relative-luminance formula) | **Selected** | REQ 2 numeric acceptance: measured 5.85:1 dark, 4.76:1 light |
| Viewport emulation (resize) | **Selected** | REQ 4 mobile 44px bar and REQ 5 desktop layout |
| Console/network error capture | **Selected** | REQ 5 "no console errors" — 0 errors, 0 warnings on home and cli pages |
| Screenshot capture | **Selected** | Visual evidence persisted to temp dir |
| Production build (`pnpm run docs:build`) | **Selected** | REQ 5 S1 acceptance: exit 0 re-run |
| Static source inspection (grep/diff) | **Selected** | Supporting-only evidence (single `.gradient-text` source, zero anchor changes in diff). Never the sole basis for PASS |
| A11y-tree snapshot | **Selected** | Page structure context for roles/tablists; numeric acceptance via computed styles |
| API / network-layer testing | **Rejected (N/A)** | Change is CSS-only static docs site; no API surface in scope |
| Persistence / data-layer | **Rejected (N/A)** | No data layer in scope |
| Internationalization / locale | **Rejected (N/A)** | Docs site is English-only; change touches visual tokens only |
| Security / unauthorized-access scenarios | **Rejected (N/A)** | No auth or privilege surface; static content site |
| Physical-device touch testing | **Unavailable** | Headless browser only; viewport emulation used instead |

## 5. Scenario Matrix (11/11 tested)

### REQ 1 — Hero Remains Visible And Static Under Reduced Motion

| # | Scenario | Status | Evidence |
|---|----------|--------|----------|
| 1.1 | Reduced-motion user sees a visible, static hero | **PASS** | `page.emulateMedia({reducedMotion:'reduce'})` + reload on home: `.badge-dot` `animation-name: none` (pulse killed), `.robot-glow` `none` (glow-pulse killed), `.robot` `none` (float killed), `.hero-copy` + `.hero-visual` `animation-name: none` **and `opacity: 1`** (fadeInUp killed + opacity restored), `html { scroll-behavior: auto }`. Screenshot: `qa-home-reduced.png`. **Normal case preserved**: `no-preference` + reload → all 4 animations ACTIVE (`fadeInUp` caught mid-flight at opacity 0.935/0.802, `pulse`, `glow-pulse`, `float`), `scroll-behavior: smooth`. Screenshot: `qa-home-desktop-normal.png` |
| 1.2 | Hover transforms remain available | **PASS** | Real `page.hover()` on `.tech-badge` under reduce: color `rgb(154,167,179)`→`rgb(255,255,255)`, bg `rgba(18,18,24,0.6)`→`rgb(18,18,24)`, `transition-duration: 0.15s` fires — user-initiated interactivity intact. Live dump of the reduce media query shows exactly 3 rules: `html{scroll-behavior:auto}`, the 4-animation kill, and `opacity:1;animation:none` — **zero `transform`/`:hover` declarations**, so `.card:hover{translateY(-2px)}` (custom.css:250) is untouched. (Verify's SUGGESTION re `.card` not rendered — see F1.) |

### REQ 2 — Footer Muted Text Meets AA Contrast In Dark Theme

| # | Scenario | Status | Evidence |
|---|----------|--------|----------|
| 2.1 | Footer dark text passes AA | **PASS** | Dark theme (`starlight-theme=dark`): footer bg `rgb(13,13,18)` (`#0d0d12`), tagline and copy color `rgb(133,142,154)` = `#858e9a`; measured contrast **5.85:1** (≥ 4.5:1) for both tagline and copy — WCAG AA. Token computed live `--as-text-muted: #858e9a` |
| 2.2 | Light theme stays untouched | **PASS** | Light theme (`starlight-theme=light` + reload): footer bg `rgb(255,255,255)`, tagline and copy `rgb(100,116,139)` = `#64748b`; measured **4.76:1**. Token computed `--as-text-muted: #64748b` — unchanged |

### REQ 3 — Single Source For The Gradient Text Utility

| # | Scenario | Status | Evidence |
|---|----------|--------|----------|
| 3.1 | No duplicated definitions | **PASS** | `rg -n "gradient-text" website/docs/src` → exactly 2 hits: the single definition (`custom.css:230`) and the class usage (`Footer.astro:13`). Zero definitions in `Footer.astro`; git diff confirms the 6-line scoped block deleted (8 lines removed in Footer.astro) |
| 3.2 | Footer logo keeps its gradient | **PASS** | Live computed on `.as-footer-logo`: `background-image: linear-gradient(135deg, rgb(0,229,196), rgb(124,77,255))`, `-webkit-text-fill-color: transparent`, `background-clip: text` — gradient rendered from the global utility. Site title and header link render the same gradient (utility intact site-wide) |

### REQ 4 — Interactive Controls Expose 44px Touch Targets

| # | Scenario | Status | Evidence |
|---|----------|--------|----------|
| 4.1 | Mobile controls meet the 44px bar | **PASS** | Viewport 375×812 on `/agentsync/reference/cli/`: search button **44.0px** (`min-height: 2.75rem`); theme select — desktop-header instance carries `min-height: 44px` (hidden on mobile via `sl-hidden md:sl-flex`), the mobile-visible instance (`.mobile-preferences`) measures **48px** ≥ 44px; 68 tabs `[role="tab"]` **min = max = 44px**, all ≥ 44px. Click on 3 tablists: `aria-selected` flips and the panel swaps (tab usability confirmed) |
| 4.2 | Anchor links keep their hit area | **PASS** | git diff: **zero** anchor/`::after` changes (remediation added no sizing rule). Live on `.sl-anchor-link` (Starlight theme anchor, `/reference/cli/`): computed `min-height: 0px`, `min-width: 0px`, `padding: 0px` — no new sizing — while the existing expanded `::after` hit area is intact: `position: absolute; inset: -4px -8px` → ~45×50px hit box (WCAG 2.2 AA expanded target). Click navigates: scrolls to `#logging-and-diagnostics`, hash updates |

### REQ 5 — Build And Desktop Layout Do Not Regress

| # | Scenario | Status | Evidence |
|---|----------|--------|----------|
| 5.1 | Docs build passes | **PASS** | QA re-ran `pnpm run docs:build` (repo root): **exit code 0**, 16 pages built, Pagefind index built, sitemap created, `[build] Complete!` |
| 5.2 | Desktop layout tolerates taller tabs | **PASS** | Desktop 1280×800: home header 1272×64, hero 1080×640 with copy (x=120, y=193) and visual (x=752, y=223) side-by-side inside hero bounds, footer centered 1080px with intact content, **no horizontal page overflow**, 0 console errors/0 warnings (home and cli). `/reference/cli/`: 68 tabs at 44px, all clickable, active panel renders; `.tablist-wrapper` computed `overflow-x: auto` (horizontal-scroll capability preserved — no current content overflows at 375/500/1280px). Screenshots: `qa-cli-desktop-tabs.png`, `qa-cli-mobile-tabs.png` |

## 6. Untested Scope, Reason, Rerun Prerequisite

**None of the 11 spec scenarios left untested — coverage 11/11 across all 5 requirements.**

Non-applicable categories recorded in the capability inventory (API, persistence, i18n, security): the change is a CSS-only visual remediation of a static docs site; no such user surface exists to exercise.

Partial coverage notes (not compliance gaps):
- The literal `.card:hover translateY(-2px)` transform was never observed on a real element (no `.card` rendered on any page). Static proof (zero transform/`:hover` declarations in the reduce block) + live hover-interactivity proof under reduce stand in. Rerun prerequisite if a `.card`-rendering page is ever added: re-hover that element under reduce and confirm the transform applies.
- Horizontal tab scroll was not triggered (no content overflows at tested widths). Rerun prerequisite: a page with a tablist wider than its container (e.g., 320px viewport with long tab labels) — confirm `scrollLeft` changes and all tabs stay clickable.

## 7. Findings

| Severity | Status | Location | Detail |
|----------|--------|----------|--------|
| P3 (INFO) | Open — non-blocking | custom.css:250 / Hero.astro | `.card:hover translateY(-2px)` not live-exercised: no `.card` element renders on any page. Proven safe via live dump of the reduce block (zero transform/`:hover` declarations) and live hover transition firing under reduce. Carried from verify (SUGGESTION). No user-facing impact |
| P3 (INFO) | Open — non-blocking | custom.css:174-179 / Starlight header | Theme-select renders two instances (desktop header + mobile menu). On mobile the desktop instance is `sl-hidden`; the visible instance measures 48px. Both carry `min-height: 44px`. Documented to prevent confusion in future mobile measurements |
| P3 (INFO) | Open — non-blocking | design.md:65 / tasks.md:27 | Doc line drift (`custom.css:22` vs actual `:23` for the dark muted token). Carried from verify. No behavioral impact |

No CRITICAL, P0, P1, or P2 findings. Nothing breaks acceptance.

## 8. Final Verdict

**PASS**

## 9. Verdict Rationale and Implementation Handoff

**Rationale**: All 11 acceptance scenarios from the spec were exercised against the running site and passed with numeric, observable evidence. The user-facing promises hold in practice:

- Reduced-motion users get a fully visible (`opacity: 1`) static hero with all four entrance/infinite animations disabled, and hover interactivity still responds; the majority (no-preference) still sees the animated hero.
- Dark-mode footer text now measures 5.85:1 (was a failing ~4.01:1 per baseline diff), light mode untouched at 4.76:1.
- Footer logo and site title keep their gradient from a single utility definition; no duplication remains.
- On a 375px mobile viewport, search, theme select, and all 68 tabs meet the 44px bar, and tabs switch panels on click; heading anchor links keep their expanded hit area untouched.
- The docs build passes (exit 0, 16 pages) and the desktop layout shows no regressions (hero/footer intact, no horizontal overflow, zero console errors, tab overflow capability preserved).

**Implementation handoff**: No fixes required. The two verify-era SUGGESTIONs (`.card` hover exercise, mobile-viewport measurement) were resolved in this QA (mobile measured; `.card` proven safe statically + interactivity proven live) and are now closed as P3 INFO. Optional follow-ups only: fix the doc line drift (design.md:65 / tasks.md:27) when next editing those files; re-hover a `.card` element under reduce if one is ever rendered, to close the loop with literal transform evidence. The change is ready for `sdd-archive`.

## Evidence Index

Screenshots (temp dir, outside repo):
- `qa-home-desktop-normal.png` — home, desktop, no-preference (animated hero)
- `qa-home-reduced.png` — home, desktop, reduce (static visible hero)
- `qa-cli-desktop-tabs.png` — reference/cli, desktop 1280px, 44px tabs
- `qa-cli-mobile-tabs.png` — reference/cli, mobile 375px, 44px tabs

All numeric measurements were captured live in-browser (computed styles, `getBoundingClientRect`, WCAG relative-luminance formula) against `http://localhost:4321`.
