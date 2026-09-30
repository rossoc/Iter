# 04 – Direction for Iter

**Status: built** (`iter serve`), with the differences noted below. The
sheet is `/v2.css`: `src/web/v2.css` (tokens, themes, links, focus, motion)
plus one `ui/<component>.css` per component, concatenated. Constraints kept:
**no web-font CDN, no JS beyond `board.js` and `org.js`.**

**Mood:** *calm workshop.* Warm neutrals, one ink-blue accent, precise
type, hairlines. It should feel like a well-made paper planner rendered by
Linear.

## 1. Tokens

Built as follows (`src/web/v2.css`). The tokens are scoped on the `.v2`
wrapper that `frame` writes, not on `:root`; only `--bg` is on `:root`
(the body outside `.v2` paints it). The layout tokens sit on `html`, because
`scroll-padding-top` can only be set there.

```css
:root { --bg: #fbfaf8; }                         /* page (dark: #24283b) */
.v2 {
  color-scheme: light dark;                      /* native controls follow the theme */
  --bg-2:#f4f2ee; --bg-3:#ebe8e2;                /* cards and panels; hover, chips */
  --line:#e4e0d8; --line-2:#d3cec4;              /* hairline; dashed boxes, separators */
  --control-line:#857f76;                        /* border of a control and of a link's rest line: 3:1 or more */
  --fg:#1d1c1a; --muted:#6f6b64; --faint:#76716a;/* muted is all text; faint only decoration (separators, icons) */
  --accent:#3b5bdb; --accent-tint:color-mix(in oklab, var(--accent) 12%, var(--bg)); --on-accent:#fff;
  --ok:#14783a; --danger:#b42318;
  --font:"Inter","InterVariable",system-ui,...;  --mono:ui-monospace,"JetBrains Mono",...;
  --shadow; --shadow-lift;
  --r-sm:4px; --r-md:8px; --r-lg:12px; --r-pill:999px;
  --ease-out:cubic-bezier(.22,1,.36,1);  --fast:150ms; --med:250ms; --slow:350ms;
  --line-w:2px;                                  /* the accent line of links and tabs */
  --touch:44px; --main-w:1180px; --main-pad-max:72px; --main-pad:clamp(20px,5vw,72px); --measure:68ch;
}
```

**Dark theme: Tokyo Night**, matching the terminal, not the near-black with
a blue accent first proposed: `--bg #24283b`, `--bg-2 #1f2335`, `--bg-3
#292e42`, `--line #32344a`, `--line-2 #444b6a`, `--control-line #737aa2`,
`--fg #a9b1d6`, `--muted #9699a8`, `--faint #8c90a3`, `--accent #ff9e64` (the
orange), `--on-accent #24283b`, `--ok #9ece6a`, `--danger #f7768e`, no shadow.
It follows `prefers-color-scheme`; there is no `data-theme` switch.

**Tokens that exist beyond that** (each set on the component that uses it, or
on `html` for the layout ones): `--head-gap` (space under a page header and
tabs, 36px), `--edge-w` (4px) with `--q1` / `--q2` (the two halves of a
priority edge), `--form-gap`, `--num-w`, `--quad-gap`, `--quad-min`,
`--info-aside`, `--info-gap`, `--info-body` (derived from `--main-w`),
`--label-w` (hour labels), `--control-h`; layout on `html`: `--bar-h` (top bar,
52px), `--sticky-head-h` (76px), `--pickbar-h` (164px), `--sticky-h` (what a
page keeps sticky under the bar; 0 when narrow or short), `--stick-top`
(`--bar-h` + 24px, where an aside sticks). Breakpoints stay literal (a media
query cannot read a token); the list is a comment at the top of `v2.css`.

**Not built:** the `--s1..--s7` spacing scale (spacing is literal, on a 4px
base), the `--t-*` type scale (about a dozen literal sizes: `.8125rem` and
`.875rem` are the common ones; a swap to four tokens would change values, so
it was left), `--ease-std`, the grain texture, `@view-transition`, `<kbd>`,
and `form:has(:invalid)`.

Components never name a color directly: they read tokens.

## 2. Typography

- Body 15px/1.55 (no `--t-*` scale, see section 1), with
  `font-feature-settings: "cv11", "ss01"` if Inter is installed locally.
  Otherwise the system font is fine. **Don't fetch fonts.**
- Headings: `font-weight: 600; letter-spacing: -.02em; text-wrap: balance`.
  The page `h1` is `clamp(2rem, 1.4rem + 2vw, 3rem)`, 650, `-.035em`.
- Labels and eyebrows (breadcrumb, section labels, table `th`): the one
  `.label` class, 11px (`.6875rem`), 600, `uppercase`, `letter-spacing: .09em`,
  `color: var(--muted)`.
- **All times, durations and counts**: `font-family: var(--mono);
  font-variant-numeric: tabular-nums`. The board's `.slot` and the agenda's
  `.label` both qualify.
- Line length: prose is `max-width: var(--measure)` (68ch); `main` is `--main-w` (1180px). Boards use the full width.

## 3. Components

**Links.** In content, a 1px `--control-line` line at rest (so a link reads
as one without hover); on hover or focus a 2px accent line slides in from the
left over it (background size animates), and out to the right on leave.
The nav bar and breadcrumb links have no line. Forced colors: underline.

**Sidebar.** Not built as proposed and since removed: there is a top bar with
the brand (a link to Home) and the nav (Board), the breadcrumb and tabs under it. The
current nav item is `--accent-tint` with `--accent` text.

**Top header.** Sticky, translucent and frosted:
```css
header.top { position: sticky; top: 0; z-index: 10;
  background: color-mix(in srgb, var(--bg) 80%, transparent);
  backdrop-filter: saturate(1.4) blur(12px); }
```

**Tabs.** An underline that slides: it sits under the current tab; hovering
or focusing another moves it there (the current line slides out to the right,
the new one in from the left; `--slow`, `--ease-out`).

**Pills and status.** Replace the text pills with glyph plus text:
`○ queue`, `◐ wip` (accent), `✓ done` (ok). The glyphs are inline SVG.

**Buttons.** Primary: accent fill, `--r-md`, `padding: 6px 14px`,
`font-weight: 550`. Secondary: `--bg-2` fill plus a `--line-2` border.
`:active { transform: translateY(1px) }`. `:focus-visible { outline: 2px
solid var(--accent); outline-offset: 2px }` (an outline everywhere, also on
controls). The secondary border is `--control-line`, not `--line-2`.

**Inputs.** `border: 1px solid var(--line-2); border-radius: var(--r-md)`.
On focus: the accent outline as everywhere. The border is `--control-line`.

**Cards (board).**
```css
.card { background: var(--bg); border: 1px solid var(--line);
  border-left: 4px solid var(--q, var(--line-2));  /* --edge-w */
  border-radius: var(--r-md); box-shadow: var(--shadow);
  transition: transform var(--fast) var(--ease-out), box-shadow var(--fast) var(--ease-out); }
.card:hover { transform: translateY(-1px); box-shadow: var(--shadow-lift); }
.card.u  { --q: var(--urgent); }
.card.i  { --q: var(--important); }
.card.ui { --q: var(--both); }
```
A colored **left edge** instead of a 35% fill (the Amie and Notion Calendar
pattern) keeps the board readable when it's full. *Implemented:* 4px (`--edge-w`), not 3px, and a card with both flags shows the two colors one above the other.

**Matrix.** Number the quadrants, as in
`1 Do first · 2 Plan · 3 Delegate · 4 Eliminate` (the numbered-section
pattern from 01; "Plan", not "Schedule", so the word stays the agenda's
verb). *Implemented (`/board/{id}/matrix`):* no fills at rest. Each quadrant
is a hairline box with a 4px **top edge** in the board's priority colors
(the same colors as a card's 4px left edge, `ui/edge.css`), a circled 1-4
numeral, the name (600, -.015em, as a card group's heading), a count and its
rule as a small label ("Important, urgent"). The inner list has no box while
it holds cards; it is dashed when empty, and dashed with the accent tint
while a card is over it (`.over`).

**Agenda.** Hour rows separated by hairlines only, with the hour label in
mono. Add a **now line** positioned by the server:
`<div class="now" style="--at: 0.42">`, where `top: calc(var(--at) *
100%)`, a 2px accent line and an 8px dot (`--dot`). This needs no JS.

**Reports.** Use Gothub's proportion bar for time per task:
```html
<div class="bar"><span style="width:62%;--c:var(--accent)"></span><span style="width:38%;--c:var(--faint)"></span></div>
```

**Empty states.** Two shapes, both `--muted` text: a quiet line
(`empty_line`, "No tasks yet.") and, for a whole page with nothing on it, a
box with a dashed `--line-2` border, a 28px icon and a hint, with no action
link (Home, Boards). Wording: "No X yet." for lists that fill up; "Nothing to
do first." only in the matrix zones.

**Texture (optional).** *Not built.* A barely-there grain on `body::before`
(inline SVG `feTurbulence`) was proposed and left out.

## 4. Motion

- Hover and press: `var(--fast)` with `var(--ease-out)`. Panels and
  dialogs: `var(--med)`.
- Animate `transform` and `opacity` only.
- Everything behind: under `prefers-reduced-motion: reduce` every
  `transition-duration` is 0 (`v2.css`). The animations (the open session's
  dot, the report's flash) exist only under `no-preference`, so they need no
  reset.
- `@view-transition { navigation: auto; }`: *not built.*

## 5. Layout

- The sidebar grid was not built (see Components): one column, `main` at
  `--main-w` with `--main-pad` side padding; below 900px two-column parts
  stack and sticky parts turn static.
- Spacing is literal, not from `--s*` tokens (not built).
- `:has()` is used for the sticky tokens (`html:has(.sticky-head)`) and the
  tabs; `form:has(:invalid)` is not built.

## 6. Don'ts

Don't add a preloader, a custom cursor, scroll hijacking (Lenis), WebGL,
a web-font CDN or an icon font. Don't use emoji as UI icons, and don't put
color on anything that has no meaning.
