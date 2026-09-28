# 05 – SVG icons (copy and paste, no dependencies)

Rule: **no icon fonts, no npm packages and no `<script>` loaders.** Open the
site, copy the `<svg>` for each icon and paste it into the Rust view or the
CSS. All sites below loaded fine on 2026-09-28, and the licenses were read
from each project's repository.

## Where to get them

| Library | Browse / copy | License | Style | Notes |
|---|---|---|---|---|
| **Lucide** ⭐ recommended | [lucide.dev/icons](https://lucide.dev/icons/) | ISC | 24px stroke, 2px, round caps | About 1.6k icons. A fork of Feather that's actively maintained, with a good set of time and calendar icons (`calendar-clock`, `timer`, `hourglass`). |
| Tabler Icons | [tabler.io/icons](https://tabler.io/icons) | MIT | 24px stroke, adjustable width | The biggest stroke set (about 5k), with outline and filled variants. |
| Phosphor | [phosphoricons.com](https://phosphoricons.com) | MIT | 6 weights (thin → fill, duotone) | Good when you want one icon family in several weights. |
| Heroicons | [heroicons.com](https://heroicons.com) | MIT | 24 outline, 24 solid, 20 mini, 16 micro | Small, very polished set from the Tailwind team. |
| Iconoir | [iconoir.com](https://iconoir.com) | MIT | 24px stroke, 1.5px | Lighter and more elegant stroke. |
| Radix Icons | [radix-ui.com/icons](https://www.radix-ui.com/icons) | MIT | 15px crisp | Pixel-perfect at small sizes, good for dense tables. |
| Feather | [feathericons.com](https://feathericons.com) | MIT | 24px stroke | The original. Frozen now, so prefer Lucide. |
| Akar Icons | [akaricons.com](https://akaricons.com) | MIT | Rounded stroke | A friendly look. |
| Octicons | [primer.style/octicons](https://primer.style/octicons/) | MIT | 16/24px | GitHub's own set: `git-branch`, `issue-opened`, `issue-closed`, matching Iter's GitHub integration. |
| Material Symbols | [fonts.google.com/icons](https://fonts.google.com/icons) | Apache-2.0 | Configurable (weight, fill, grade) | Use the "SVG" download button, *not* the font. |
| Bootstrap Icons | [icons.getbootstrap.com](https://icons.getbootstrap.com) | MIT | 16px | Every page has a "copy HTML" box. |
| Simple Icons | [simpleicons.org](https://simpleicons.org) | CC0 (the marks remain trademarks) | Brand logos | For GitHub, Codeberg and tmux logos. |
| Remix Icon | [remixicon.com](https://remixicon.com) | Remix Icon License v1.0 (custom) | Line and fill | Read the license before shipping. |
| Material Design Icons (Pictogrammers) | [pictogrammers.com/library/mdi](https://pictogrammers.com/library/mdi/) | Pictogrammers Free License | Huge community set | The license is custom but GPL-friendly. |

**Search across all of them:** [icones.js.org](https://icones.js.org). It's
a browser for 150+ Iconify sets. Pick an icon and use **"Copy SVG"**, or
**"Copy CSS"**, which gives a `mask-image` data URI (see below).
[yesicon.app](https://yesicon.app) and [iconbuddy.com](https://iconbuddy.com)
do the same. The license is still the license of the *set* the icon came
from.

**Pick one family and stick to it.** Mixing stroke widths is the fastest
way to look unpolished. Lucide for UI, plus Simple Icons for brand logos.

## Keeping attribution

MIT and ISC ask for the copyright notice to be kept. Add one comment next to
the icons, for example `// Icons: Lucide (ISC), https://lucide.dev/license`.
A `NOTICE` file in the repo also works.

## Three ways to embed

### A. Inline `<svg>` (best: themeable, crisp, accessible)

Size and color come from CSS through `currentColor`:

```css
.i { width: 1em; height: 1em; flex: none; vertical-align: -.125em;
     fill: none; stroke: currentColor; stroke-width: 2;
     stroke-linecap: round; stroke-linejoin: round; }
```
```html
<svg class="i" viewBox="0 0 24 24" aria-hidden="true"><path d="M20 6 9 17l-5-5"/></svg>
```
Because the stroke attributes live in the CSS, each icon is just
`viewBox` plus paths. In Rust, keep them as `const` strings in one
`icons.rs` and write a tiny `icon(name)` component.

### B. One sprite, many `<use>` (best when icons repeat)

Emit this once, at the top of `<body>` in `layout.rs`:
```html
<svg width="0" height="0" style="position:absolute">
  <symbol id="i-check" viewBox="0 0 24 24"><path d="M20 6 9 17l-5-5"/></symbol>
  <!-- … -->
</svg>
```
Then use `<svg class="i"><use href="#i-check"/></svg>` anywhere. The
sidebar repeats the same folder icon for every project, so this keeps the
HTML small.

### C. CSS `mask-image` (for decorative icons in pseudo-elements)

```css
.ico-cal::before { content: ""; width: 1em; height: 1em; background: currentColor;
  -webkit-mask: var(--svg) center / contain no-repeat; mask: var(--svg) center / contain no-repeat;
  --svg: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M8 2v3M16 2v3'/%3E%3Crect x='3' y='3' width='18' height='18' rx='2'/%3E%3Cpath d='M3 9h18'/%3E%3C/svg%3E"); }
```
The color comes from `background: currentColor`. It stays pure CSS, but
screen readers can't see it, so use it only for decoration.

## Starter set for Iter (Lucide, ISC)

Copied from `lucide-icons/lucide@main/icons/*.svg`. Use these with the `.i`
class above, which already sets the stroke.

| Use | Name | Inner markup (`viewBox="0 0 24 24"`) |
|---|---|---|
| Organization | `building` | `<rect x="4" y="2" width="16" height="20" rx="2"/><path d="M9 22v-3a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v3"/><path d="M8 6h.01M12 6h.01M16 6h.01M8 10h.01M12 10h.01M16 10h.01M8 14h.01M12 14h.01M16 14h.01"/>` |
| Project | `folder` | `<path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/>` |
| Tasks | `list-todo` | `<path d="M13 5h8"/><path d="M13 12h8"/><path d="M13 19h8"/><path d="m3 17 2 2 4-4"/><rect x="3" y="4" width="6" height="6" rx="1"/>` |
| Board | `kanban` | `<path d="M5 3v14"/><path d="M12 3v8"/><path d="M19 3v18"/>` |
| Matrix | `layout-grid` | `<rect width="7" height="7" x="3" y="3" rx="1"/><rect width="7" height="7" x="14" y="3" rx="1"/><rect width="7" height="7" x="14" y="14" rx="1"/><rect width="7" height="7" x="3" y="14" rx="1"/>` |
| Agenda / day | `calendar` | `<path d="M8 2v3"/><path d="M16 2v3"/><rect x="3" y="3" width="18" height="18" rx="2"/><path d="M3 9h18"/>` |
| Start time | `calendar-clock` | `<path d="M16 14v2.2l1.6 1"/><path d="M16 2v3"/><path d="M21 7.338V5a2 2 0 00-2-2H5a2 2 0 00-2 2v14a2 2 0 002 2h2.338"/><path d="M3 9h5.859"/><path d="M8 2v3"/><circle cx="16" cy="16" r="6"/>` |
| Duration | `timer` | `<line x1="10" x2="14" y1="2" y2="2"/><line x1="12" x2="15" y1="14" y2="11"/><circle cx="12" cy="14" r="8"/>` |
| Session time | `clock` | `<circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/>` |
| Queue / inbox | `inbox` | `<polyline points="22 12 16 12 14 15 10 15 8 12 2 12"/><path d="M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z"/>` |
| Start session | `play` | `<path d="M5 5a2 2 0 0 1 3.008-1.728l11.997 6.998a2 2 0 0 1 .003 3.458l-12 7A2 2 0 0 1 5 19z"/>` |
| Stop session | `square` | `<rect width="18" height="18" x="3" y="3" rx="2"/>` |
| Done | `check` | `<path d="M20 6 9 17l-5-5"/>` |
| Urgent | `flame` | `<path d="M12 3q1 4 4 6.5t3 5.5a1 1 0 0 1-14 0 5 5 0 0 1 1-3 1 1 0 0 0 5 0c0-2-1.5-3-1.5-5q0-2 2.5-4"/>` |
| Important | `star` | `<path d="M11.525 2.295a.53.53 0 0 1 .95 0l2.31 4.679a2.123 2.123 0 0 0 1.595 1.16l5.166.756a.53.53 0 0 1 .294.904l-3.736 3.638a2.123 2.123 0 0 0-.611 1.878l.882 5.14a.53.53 0 0 1-.771.56l-4.618-2.428a2.122 2.122 0 0 0-1.973 0L6.396 21.01a.53.53 0 0 1-.77-.56l.881-5.139a2.122 2.122 0 0 0-.611-1.879L2.16 9.795a.53.53 0 0 1 .294-.906l5.165-.755a2.122 2.122 0 0 0 1.597-1.16z"/>` |
| Branch | `git-branch` | `<path d="M15 6a9 9 0 0 0-9 9V3"/><circle cx="18" cy="6" r="3"/><circle cx="6" cy="18" r="3"/>` |
| Edit | `pencil` | `<path d="M21.174 6.812a1 1 0 0 0-3.986-3.987L3.842 16.174a2 2 0 0 0-.5.83l-1.321 4.352a.5.5 0 0 0 .623.622l4.353-1.32a2 2 0 0 0 .83-.497z"/><path d="m15 5 4 4"/>` |
| New | `plus` | `<path d="M5 12h14"/><path d="M12 5v14"/>` |
| Drag handle | `grip-vertical` | `<circle cx="9" cy="12" r="1"/><circle cx="9" cy="5" r="1"/><circle cx="9" cy="19" r="1"/><circle cx="15" cy="12" r="1"/><circle cx="15" cy="5" r="1"/><circle cx="15" cy="19" r="1"/>` |
| Prev / next day | `chevron-left` / `chevron-right` | `<path d="m15 18-6-6 6-6"/>` / `<path d="m9 18 6-6-6-6"/>` |
| Theme toggle | `sun` / `moon` | `<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2m-7.07-17.07 1.41 1.41m11.32 11.32 1.41 1.41M2 12h2m16 0h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41"/>` / `<path d="M20.985 12.486a9 9 0 1 1-9.473-9.472c.405-.022.617.46.402.803a6 6 0 0 0 8.268 8.268c.344-.215.825-.004.803.401"/>` |

Note: `building` merges its nine window dots into one path, and `sun` merges
its eight rays into one path with relative moves. Both draw the same shape
as the upstream files.

**GitHub mark** (Simple Icons, CC0). It's a *filled* icon, so override the
`.i` defaults with `style="fill:currentColor;stroke:none"`:
```html
<svg class="i" viewBox="0 0 24 24" style="fill:currentColor;stroke:none" aria-hidden="true"><path d="M12 .297c-6.63 0-12 5.373-12 12 0 5.303 3.438 9.8 8.205 11.385.6.113.82-.258.82-.577 0-.285-.01-1.04-.015-2.04-3.338.724-4.042-1.61-4.042-1.61C4.422 18.07 3.633 17.7 3.633 17.7c-1.087-.744.084-.729.084-.729 1.205.084 1.838 1.236 1.838 1.236 1.07 1.835 2.809 1.305 3.495.998.108-.776.417-1.305.76-1.605-2.665-.3-5.466-1.332-5.466-5.93 0-1.31.465-2.38 1.235-3.22-.135-.303-.54-1.523.105-3.176 0 0 1.005-.322 3.3 1.23.96-.267 1.98-.399 3-.405 1.02.006 2.04.138 3 .405 2.28-1.552 3.285-1.23 3.285-1.23.645 1.653.24 2.873.12 3.176.765.84 1.23 1.91 1.23 3.22 0 4.61-2.805 5.625-5.475 5.92.42.36.81 1.096.81 2.22 0 1.606-.015 2.896-.015 3.286 0 .315.21.69.825.57C20.565 22.092 24 17.592 24 12.297c0-6.627-5.373-12-12-12"/></svg>
```

## Accessibility

- Decorative icon next to text: `aria-hidden="true"`.
- Icon-only button: put the label on the button, as in
  `<button aria-label="Start session"><svg class="i" aria-hidden="true">…</svg></button>`.
- Status glyphs (○ ◐ ✓) still need visible or visually-hidden text.
