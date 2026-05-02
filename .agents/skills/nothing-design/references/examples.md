# Nothing Design System — Output Examples

Use these as **response-shape examples**, not rigid templates. Preserve the design language while
adapting to user goals, platform constraints, and accessibility needs.

---

## 1. DASHBOARD HERO METRIC (WEB)

### Fonts required

- `Space Grotesk`
- `Space Mono`
- `Doto` for the hero metric

### Mode

- Start in **dark mode**

### Three-layer hierarchy

- **Primary:** Main throughput value (`36GB/s`)
- **Secondary:** Supporting module label and short explanatory context
- **Tertiary:** Timestamp, status, and navigation metadata

### Layout strategy

- Left-heavy composition
- Vast top spacing, tight label-to-metric spacing
- Single expressive break: oversized Doto metric floating without a card container

### Component mapping

- Hero metric block
- Stat rows
- Segmented progress bar
- Top metadata strip

### Token mapping

- Metric: `--display-xl`, `--text-display`, `Doto`
- Labels: `--label`, `--text-secondary`, `Space Mono`
- Body: `--body`, `--text-primary`, `Space Grotesk`
- Spacing: `--space-sm`, `--space-md`, `--space-2xl`, `--space-4xl`

### Example implementation direction

```html
<section class="nd-panel">
  <div class="nd-meta">[ NETWORK ] <span>UPDATED 09:41 UTC</span></div>
  <div class="nd-hero">
    <p class="nd-label">THROUGHPUT</p>
    <h1 class="nd-metric">36<span>GB/S</span></h1>
    <p class="nd-body">Primary transfer lane is operating above baseline with stable packet loss.</p>
  </div>

  <div class="nd-progress-block">
    <div class="nd-progress-copy">
      <span class="nd-label">CAPACITY</span>
      <span class="nd-value nd-value--warning">84%</span>
    </div>
    <div class="nd-segments" aria-label="Capacity at 84 percent">
      <span class="is-filled"></span><span class="is-filled"></span><span class="is-filled"></span>
      <span class="is-filled"></span><span class="is-filled"></span><span class="is-filled"></span>
      <span class="is-filled"></span><span class="is-filled"></span><span></span><span></span>
    </div>
  </div>
</section>
```

### Why this works

- The primary value dominates immediately
- Secondary support stays close and readable
- Tertiary metadata stays out of competition by size, casing, and placement

---

## 2. SETTINGS SCREEN (MOBILE)

### Fonts required

- `Space Grotesk`
- `Space Mono`

### Mode

- Start in **light mode**

### Three-layer hierarchy

- **Primary:** Section title
- **Secondary:** Setting names and active values
- **Tertiary:** Explanatory captions and navigation hints

### Layout strategy

- Top-heavy stack
- Consistent row rhythm with dividers
- Single expressive break: one inverted segmented control near the top

### Component mapping

- Back button
- Section header
- Segmented control
- Toggle rows
- Inline status text

### Token mapping

- Title: `--display-md`
- Rows: `--body` + `--label`
- Dividers: `--border`
- Controls: 44px minimum touch target

### Example response shape

```md
Fonts required: Space Grotesk, Space Mono
Mode: Light

Hierarchy:
- Primary: SETTINGS
- Secondary: toggle labels, selected mode, connectivity values
- Tertiary: helper descriptions and sync metadata

Layout:
- Back button pinned top-left
- Title near top with 64px breathing room below nav
- Display mode segmented control first
- Settings rows follow with full-width dividers

Components:
- Circular back button
- Segmented control for LIGHT / DARK / AUTO
- Toggle rows with right-aligned switch
- Inline save status: [SAVED]
```

### Why this works

- The page feels technical and calm, not app-store generic
- The segmented control provides the one high-contrast moment
- Rows stay highly scannable without cards or shadows

---

## 3. EMPTY STATE (WEB APP)

### Fonts required

- `Space Grotesk`
- `Space Mono`

### Mode

- Start in **dark mode**

### Three-layer hierarchy

- **Primary:** Empty-state headline
- **Secondary:** One-sentence explanation
- **Tertiary:** Action label or system hint

### Layout strategy

- Centered composition only because the page has no data yet
- 96px+ surrounding whitespace
- Single expressive break: subtle dot-grid illustration behind the headline

### Component mapping

- Empty state text stack
- Ghost action button
- Dot-grid motif

### Example copy

```text
[ ARCHIVE ]
NO CAPTURES YET
Your first synced capture will appear here once the device completes its next transfer cycle.
[REFRESH]
```

### Why this works

- It stays sparse without becoming vague
- One sentence explains the state without onboarding theater
- The bracketed action matches the mechanical voice of the system

---

## 4. WHEN TO USE / WHEN NOT TO USE

### Good fit

- Dashboards
- Device interfaces
- Settings panels
- Monitoring tools
- Technical product marketing sections

### Poor fit

- Child-oriented products
- Highly expressive brand campaigns needing multiple colors
- Dense enterprise CRUD screens that depend on many simultaneous emphasis cues
- Products where platform-native softness is a core brand requirement

---

## 5. IMPLEMENTATION REMINDERS

- Ask for mode if missing before final UI output
- Declare fonts before code
- Keep one expressive break per screen
- Do not use accent red unless there is true urgency or status meaning
- If accessibility conflicts with visual purity, accessibility wins
