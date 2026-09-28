# 01 – Award-site patterns (CSS Design Awards)

Source: [cssdesignawards.com](https://www.cssdesignawards.com/): the home
page, WOTD winners, nominees and gallery, fetched 2026-09-28. That gave
**69 sites**, mostly studio, agency and portfolio sites. Examples:
[Emotion Agency](https://emotion-agency.com), [FORMAT](https://format.obys.agency/),
[Awards Racing](https://awards.racing), [The Design Society](https://www.thedesignsociety.fr/),
[Red Collar](https://redcollar.co/), [Forge Automotive](https://forgeautomotive.co.uk/),
[Grande Charte](https://grandecharte.co), [Jesper Landberg](https://jesperlandberg.com),
[Viru](https://www.virugroup.com/en/), [Catskills](https://catskills-showcase.pages.dev/).

Counts are "number of sites whose HTML+CSS contains the feature", out of 69.

## 1. Foundations nearly everyone uses

| Feature | Sites | Takeaway for Iter |
|---|---|---|
| CSS custom properties | 69 | Tokens are universal. We have them; grow the set. |
| `display: grid` | 65 | Grid is the layout primitive. Flex is for rows of items. |
| `text-transform: uppercase` | 64 | Small uppercase labels are everywhere (we already use them for `h2`). |
| `@font-face` + `font-display: swap` | 64 / 59 | Custom type is the norm. We can use a local system stack instead (see 04). |
| `cubic-bezier(...)` | 63 | Nobody uses plain `ease`. Custom easing is a signature. |
| `aspect-ratio` | 62 | Media and cards keep their proportions. |
| `clip-path` | 59 | Used for reveals and masks, and for the visually-hidden pattern. |
| `svh` / `dvh` units | 58 | Full-height layouts that behave on mobile. |
| Negative `letter-spacing` | 54 | Headings get tightened. This is the most visible "designed" signal. |
| 1px hairline borders | 54 | Structure comes from hairlines, not heavy boxes. |
| `position: sticky` | 50 | Sticky headers, sidebars and section labels. |
| `@media (hover: hover)` | 46 | Hover effects only where a pointer exists. |
| `backdrop-filter` | 47 | Frosted translucent headers and overlays. |

## 2. Modern CSS they reach for

| Feature | Sites |
|---|---|
| `clamp()` for fluid type and space | 42 |
| `:focus-visible` | 42 |
| `mix-blend-mode` (usually `difference` on a cursor or header) | 42 |
| `prefers-reduced-motion` | 41 |
| `:has()` | 38 |
| `text-underline-offset` (styled links) | 38 |
| `color-mix()` | 30 |
| `font-feature-settings` | 29 |
| `::selection` styled | 27 |
| `tabular-nums` | 25 |
| `@property` (animatable custom props) | 23 |
| `prefers-color-scheme` | 23 |
| `text-wrap: balance` | 17 |
| `scroll-snap` | 16 |
| `subgrid`, `@container`, view transitions, `oklch()` | ≤ 6 each (still early) |

## 3. Signature "awwwards look" devices

| Device | Sites | Useful for Iter? |
|---|---|---|
| Preloader / intro | 44 | No. A local tool should be instant. |
| Custom cursor | 35 | No. It hurts usability in a work tool. |
| Numbered sections `(01)` | 28 | **Yes.** Ideal for board quadrants, steps and list indices. |
| Marquee / ticker | 27 | Rarely. Maybe a "now running" session strip. |
| Grain / noise overlay | 21 | **Maybe.** A 2–3% SVG noise on the canvas adds warmth for free. |
| Eyebrow / overline labels | 17 | **Yes.** Small uppercase context above titles. |
| Giant hero type ≥ 8vw | 12 | Only on an empty state or dashboard header. |

## 4. The numbers designers actually pick

**Easing** (sites using each curve):

| Curve | Sites | Name |
|---|---|---|
| `.4,0,.2,1` | 28 | Material "standard" |
| `0,0,.2,1` | 19 | Material "decelerate" |
| `.22,1,.36,1` | 14 | easeOutQuint |
| `.16,1,.3,1` | 12 | easeOutExpo |
| `.19,1,.22,1` | 10 | easeOutExpo (variant) |
| `.34,1.56,.64,1` | 10 | easeOutBack (a slight overshoot) |

**Durations:** `.3s` (47), `.2s` (42), `.15s` (36), `.5s` (34), `.4s` (33).
Interaction feedback lives at 150–300ms. Anything longer is choreography.

**Radii:** `50%`/`999px` for dots and pills. Otherwise **2–4px** is the most
common (≈24 sites each), then 8–12px. Award sites are *sharper* than SaaS
apps (see 03).

**Colors:** the most repeated hexes are neutrals: `#ffffff`, `#fafafa`,
`#000000`, `#f3f3f3`, `#1a1a1a`, `#0c0c0c`, `#171717`. Pure black and white
are softened into near-black and off-white.

**Fonts:** Inter (10), the Helvetica Neue and Arial fallbacks, Geist (4),
Inter Tight, Space Grotesk, PP Neue Montreal. Serif accents come from
Instrument Serif, Cormorant and Bodoni. Mono shows up a lot for metadata:
ui-monospace (8), JetBrains Mono (6), IBM Plex Mono (5) and Geist Mono (4).
The common pairing is a **neutral grotesk plus a mono for labels**, sometimes
with an editorial serif for one accent word.

**JS (for context only, not for Iter):** Next (19), Lenis smooth scroll (19),
GSAP (17), SplitText (12), three.js (11), Webflow (11). The effects are
JS-heavy, but the *static* layer (type, spacing, color) is what makes them
look good. That layer is plain CSS, and it's the part Iter can copy.

## 5. What to steal, in one paragraph

A near-neutral canvas with one confident accent. Tight, large headings over
small uppercase or mono labels. Hairline dividers on a grid. Numbered
sections. Generous whitespace. Links underlined with an offset. Every hover
eased with a quint or expo curve over 200–300ms. Skip the preloaders,
cursors and WebGL. Iter is a tool people keep open all day, not a
portfolio.
