# 04 – Direction for Iter

A proposal, not yet implemented. It builds on the current
`src/web/style.css` (tokens on `:root`, a dark override, `color-mix` for
quadrant tints) and keeps its constraints: **one stylesheet, no web-font
CDN, no JS beyond `board.js`.**

**Mood:** *calm workshop.* Warm neutrals, one ink-blue accent, precise
type, hairlines. It should feel like a well-made paper planner rendered by
Linear.

## 1. Tokens

```css
:root {
  color-scheme: light dark;                    /* from Gothub: native controls follow theme */

  /* surfaces: 4 lightness steps, slightly warm (never pure grey) */
  --bg:    #fbfaf8;   /* page */
  --bg-2:  #f4f2ee;   /* sidebar, header, cards */
  --bg-3:  #ebe8e2;   /* hover, pressed */
  --line:  #e4e0d8;   /* hairline */
  --line-2:#d3cec4;   /* stronger hairline, inputs */

  /* ink */
  --fg:    #1d1c1a;
  --muted: #6f6b64;
  --faint: #a19c93;

  /* one accent + its tint (Linear pattern) */
  --accent:      #3b5bdb;
  --accent-tint: color-mix(in oklab, var(--accent) 10%, var(--bg));
  --on-accent:   #fff;

  /* meaning */
  --danger: #c2410c;
  --ok:     #15803d;

  /* type */
  --font: "Inter", "InterVariable", system-ui, -apple-system, "Segoe UI", sans-serif;
  --mono: ui-monospace, "JetBrains Mono", "SF Mono", Menlo, monospace;
  --t-xs: .75rem; --t-sm: .8125rem; --t-md: .9375rem; --t-lg: 1.125rem;
  --t-xl: 1.5rem; --t-2xl: clamp(1.75rem, 1.2rem + 1.6vw, 2.5rem);

  /* space: 4px base */
  --s1: 4px; --s2: 8px; --s3: 12px; --s4: 16px; --s5: 24px; --s6: 32px; --s7: 48px;

  /* shape */
  --r-sm: 4px; --r-md: 8px; --r-lg: 12px; --r-pill: 999px;
  --shadow: 0 1px 0 rgb(0 0 0 / .03), 0 1px 3px rgb(0 0 0 / .06);
  --shadow-lift: 0 8px 24px -8px rgb(0 0 0 / .18);

  /* motion (the curves most used by the award sites) */
  --ease-out: cubic-bezier(.22, 1, .36, 1);    /* quint */
  --ease-std: cubic-bezier(.4, 0, .2, 1);
  --fast: 150ms; --med: 250ms;
}

@media (prefers-color-scheme: dark) { :root:not([data-theme="light"]) {
  --bg: #121212; --bg-2: #1a1a19; --bg-3: #232321;
  --line: #2a2a27; --line-2: #3a3935;
  --fg: #ecebe8; --muted: #9d9990; --faint: #6c6962;
  --accent: #8ea4ff; --on-accent: #0d1020;
  --shadow: none; --shadow-lift: 0 8px 24px -8px rgb(0 0 0 / .6);
}}
```

Only `:root` changes between themes. Components never name a color directly.

## 2. Typography

- Body `var(--t-md)`/1.55 (15px, as now), with
  `font-feature-settings: "cv11", "ss01"` if Inter is installed locally.
  Otherwise the system font is fine. **Don't fetch fonts.**
- Headings: `font-weight: 600; letter-spacing: -.02em; text-wrap: balance`.
  The page `h1` uses `--t-2xl`.
- Labels and eyebrows (sidebar `h2`, zone `h3`, table `th`): `--t-xs`,
  `uppercase`, `letter-spacing: .08em`, `color: var(--muted)`. Iter already
  has this; make it one reusable `.label` class.
- **All times, durations and counts**: `font-family: var(--mono);
  font-variant-numeric: tabular-nums`. The board's `.slot` and the agenda's
  `.label` both qualify.
- Line length: `main { max-width: 72ch }` for reading pages. Boards stay full-width.

## 3. Components

**Links.** `text-decoration: underline; text-decoration-color: var(--line-2);
text-underline-offset: .2em` → on hover the decoration color becomes
`currentColor`. This is the Gothub and award-site treatment, and it's
quieter than a color change.

**Sidebar.** `background: var(--bg-2)`, sticky with `height: 100dvh`.
The selected item is `background: var(--accent-tint); color: var(--accent)`
**instead of** a solid accent fill, which is calmer. Put a 16px icon before
the org and project names (05).

**Top header.** Sticky, translucent and frosted:
```css
header.top { position: sticky; top: 0; z-index: 10;
  background: color-mix(in srgb, var(--bg) 80%, transparent);
  backdrop-filter: saturate(1.4) blur(12px); }
```

**Tabs.** Keep the underline tabs, but make the underline slide in with
`transition: border-color var(--fast) var(--ease-out)`.

**Pills and status.** Replace the text pills with glyph plus text:
`○ queue`, `◐ wip` (accent), `✓ done` (ok). The glyphs are inline SVG.

**Buttons.** Primary: accent fill, `--r-md`, `padding: 6px 14px`,
`font-weight: 550`. Secondary: `--bg-2` fill plus a `--line-2` border.
`:active { transform: translateY(1px) }`. `:focus-visible { outline: 2px
solid var(--accent); outline-offset: 2px }`.

**Inputs.** `border: 1px solid var(--line-2); border-radius: var(--r-md)`.
On focus: `border-color: var(--accent); box-shadow: 0 0 0 3px
var(--accent-tint)`.

**Cards (board).**
```css
.card { background: var(--bg); border: 1px solid var(--line);
  border-left: 3px solid var(--q, var(--line-2));
  border-radius: var(--r-md); box-shadow: var(--shadow);
  transition: transform var(--fast) var(--ease-out), box-shadow var(--fast) var(--ease-out); }
.card:hover { transform: translateY(-1px); box-shadow: var(--shadow-lift); }
.card.u  { --q: var(--urgent); }
.card.i  { --q: var(--important); }
.card.ui { --q: var(--both); }
```
A colored **left edge** instead of a 35% fill (the Amie and Notion Calendar
pattern) keeps the board readable when it's full.

**Matrix.** Number the quadrants in the `.axis` labels, as in
`01 Do · 02 Plan · 03 Delegate · 04 Drop` (the numbered-section pattern
from 01). Keep the quadrant fills at about 8% at rest. On
`.zone.over`, raise the fill to about 20% and add a dashed accent border.

**Agenda.** Hour rows separated by hairlines only, with the hour label in
mono. Add a **now line** positioned by the server:
`<div class="now" style="--at: 0.42">`, where `top: calc(var(--at) *
100%)`, a 1px accent line and a 6px dot. This needs no JS.

**Reports.** Use Gothub's proportion bar for time per task:
```html
<div class="bar"><span style="width:62%;--c:var(--accent)"></span><span style="width:38%;--c:var(--faint)"></span></div>
```

**Empty states.** Show a 32px muted icon, one line of `--muted` copy and
one action, such as "Nothing queued. Enjoy it, or <a>add a task</a>."

**Texture (optional).** Put a barely-there grain on `body::before`: an
inline SVG `feTurbulence` as a data URI at `opacity: .025`. It costs
nothing and removes the flat digital look.

## 4. Motion

- Hover and press: `var(--fast)` with `var(--ease-out)`. Panels and
  dialogs: `var(--med)`.
- Animate `transform` and `opacity` only.
- Everything behind:
  ```css
  @media (prefers-reduced-motion: reduce) { *, *::before, *::after {
    transition-duration: 0s !important; animation-duration: 0s !important; } }
  ```
- Optional progressive enhancement for page navigation, with no JS:
  `@view-transition { navigation: auto; }` (Chromium only; other browsers
  ignore it). Page loads get a soft cross-fade between sidebar pages.

## 5. Layout

- `.app { display: grid; grid-template-columns: 248px 1fr; }` replaces the
  flex layout. Below 900px it becomes one column and the sidebar turns into
  a `<details>` drawer (no JS, the Gothub breakpoint).
- Spacing only from `--s*` tokens. Section rhythm is `--s6`/`--s7`.
- Use `:has()` more: `main:has(.board)` is already there. Extend the idea
  to `form:has(:invalid) button { opacity: .6 }` and similar.

## 6. Don'ts

Don't add a preloader, a custom cursor, scroll hijacking (Lenis), WebGL,
a web-font CDN or an icon font. Don't use emoji as UI icons, and don't put
color on anything that has no meaning.
