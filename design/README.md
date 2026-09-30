# Iter design wiki

Research notes for the `iter serve` web UI (`src/web/`). The goal is a
UI that is **beautiful, calm and dependency-free**: one stylesheet,
inline SVG icons, no JS frameworks, no web fonts pulled from a CDN.

**Status: built.** The stylesheet is one concatenated sheet served at
`/v2.css` (`src/web/v2.css` for tokens, themes and links, then one
`src/web/ui/<component>.css` per component); `/org.css` is the one page
sheet. Page 04 records what was built and how it differs from the proposal.

| Page                                                  | What it covers                                                                 |
| ----------------------------------------------------- | ------------------------------------------------------------------------------ |
| [01 – Award-site patterns](01-award-site-patterns.md) | What 69 recent CSS Design Awards sites have in common, measured from their CSS |
| [02 – Gothub](02-gothub.md)                           | What the no-JS, ~8 kB GitHub frontend does well and what to skip               |
| [03 – Scheduling apps](03-scheduling-apps.md)         | How Linear, Sunsama, Amie, Cal.com, Things and others present time and tasks   |
| [04 – Direction for Iter](04-direction-for-iter.md)   | The design system as proposed and as built: tokens, type, motion, components   |
| [05 – SVG icons](05-svg-icons.md)                     | Where to copy icons from (licenses checked), how to inline them, a starter set |

## The short version

1. **Tokens first.** Every award site (69/69) and nearly every scheduling
   app keeps its design in CSS custom properties. Iter does this (colors,
   radius, motion, layout); the space and type scales were not built.
2. **Neutral canvas, one accent, meaning-coded color.** Almost all of these
   sites are near-white `#fafafa` or near-black `#0c0c0c`–`#1a1a1a` surfaces
   with one accent. Color carries meaning (urgent, important, WIP), not
   decoration.
3. **Typography does the heavy lifting.** Tight negative tracking on
   headings, small uppercase labels with wide tracking, tabular numbers
   for times and durations, a mono face for metadata.
4. **Hairlines, not boxes.** 1px borders at low contrast (54/69), small
   radii (2–8px) for controls and pills for tags.
5. **Motion is short and eased.** 150–300ms with an ease-out curve such as
   `cubic-bezier(.22,1,.36,1)`, always behind `prefers-reduced-motion`.
6. **Gothub proves the point.** A good-looking UI needs neither JS nor
   external assets. Iter's one small `board.js` for drag and drop is fine.
7. **Icons are copied, not installed.** Lucide (ISC) is the recommended
   default. Icons go inline as `<svg>` with `stroke="currentColor"`.

## Method and limits

- The CSSDA pages (home, nominees, winners, gallery) were fetched on
  2026-09-28. That gave 73 external links; 69 were real award entries.
  For each one, the HTML and every linked stylesheet were downloaded and
  searched with regexes for features, fonts, easings, radii, durations
  and colors.
- 16 scheduling and productivity sites went through the same script.
  13 returned content. Rise, Height and Fantastical returned nothing.
- Static analysis only: no screenshots, and nothing rendered by JS after
  load. The counts show what these sites *ship*. They can't show which
  sites look good. The qualitative notes come from reading the pages.
- The scratch scripts are not kept in the repo. Re-running them later
  will give different counts, because CSSDA's front page changes daily.
