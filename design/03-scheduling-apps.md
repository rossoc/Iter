# 03 – Scheduling and productivity apps

Looked at: [Linear](https://linear.app/), [Sunsama](https://www.sunsama.com/),
[Amie](https://www.amie.so/), [Cal.com](https://cal.com/),
[Notion Calendar](https://www.notion.com/product/calendar),
[Motion](https://www.usemotion.com/), [Akiflow](https://akiflow.com/),
[Reclaim](https://reclaim.ai/), [Morgen](https://www.morgen.so/),
[Todoist](https://www.todoist.com/), [Things](https://culturedcode.com/things/),
[Tweek](https://tweek.so/), [SavvyCal](https://savvycal.com/).
These are the marketing sites; the product shots on them show the app UI.

## How they differ from award sites

| | Award sites (01) | Scheduling apps |
|---|---|---|
| Radius | Sharp, 2–4px | Softer: **6, 8, 12, 16px** are the most common (10 of 13 sites each) |
| Type | Expressive display faces | **Inter** everywhere (8/13); Cal Sans, DM Sans, Matter, Manrope |
| Motion | Choreographed, 0.5–1s | Snappy: **.1–.3s** |
| Frosted glass | 47/69 | 12/13. Sticky translucent headers are standard |
| `text-wrap: balance` | 17/69 | 7/13 |
| `tabular-nums` | 25/69 | 6/13. Times must line up |
| `subgrid` / container queries | ≤ 6/69 | 6 and 5 of 13. Dense UIs use them more |

In short, these apps are **soft, dense and fast**, where award sites are
**sharp, airy and slow**. Iter sits in between: it's a work tool that
should still feel crafted.

## Linear: the reference for a "beautiful tool"

These tokens come from Linear's shipped CSS. Worth studying:

- **Five background levels**: `--color-bg-primary / secondary / tertiary /
  quaternary / quinary`. In light mode they run
  `#fff → #f9f8f9 → #f4f2f4 → #eeedef → #e9e8ea` (slightly purple-tinted
  greys, never pure grey). In dark mode they run
  `#08090a → #1c1c1f → #232326 → #28282c`.
- **Three border levels**: light mode `#e9e8ea / #e4e2e4`, dark mode
  `#23252a / #34343a / #3e3e44`. In the glass theme they're alpha whites
  such as `#ffffff14` and `#ffffff1f`.
- **One accent**: `#7170ff`, with a separate hover shade and a tint for
  selected backgrounds (`#f1f1ff` light, `#18182f` dark).
- **A tight type scale in rem**: micro `.75`, mini `.8125`, small `.875`,
  regular `.9375`, large `1.125`, title3 `1.25`, title2 `1.5`, title1
  `2.25`. The body text is **15px**, the same as Iter's.
- **Fine-grained variable weights**: 400 / 510 / 590 / 680. They avoid
  jumping straight from 400 to 700.
- **A full named easing set** (`--ease-out-quint`, `--ease-in-out-cubic`…)
  as tokens.
- **Wide-gamut accents**: `@supports (color: color(display-p3 …))`
  overrides make colors punchier on good screens.

## Product concepts that map onto Iter

| Pattern | Seen in | Iter equivalent |
|---|---|---|
| **Tasks and calendar side by side**; drag a task into a time slot (timeboxing) | Sunsama, Akiflow, Motion, Amie, Notion Calendar | The board's `lists` + `agenda`. This is already the right shape. |
| **Days as columns** with the task cards for each day | Sunsama, Tweek | A week view for the agenda |
| **A daily ritual**: plan in the morning, shut down in the evening with a "time spent" review | Sunsama | A "Today" page: sessions logged today, a planned vs. actual bar |
| **Color-coded events**, soft fills and a strong left edge | Amie, Notion Calendar, Morgen | `.card` with a 3px left border in the quadrant color rather than a full fill |
| **Duration on the card** (`30m`, `1h`) | Sunsama, Motion | Iter's `duration` field, shown in mono with tabular-nums |
| **"Now" line** across the agenda | Notion Calendar, Amie, Cal.com | A 1px accent line at the current hour, with a dot |
| **Status as a small glyph**, not a word (circle → half → check) | Linear, Things | `queue` ○, `wip` ◐, `done` ✓ as inline SVG |
| **Eisenhower matrix** with quiet quadrants | (Iter's own) | Keep the 2×2, number the quadrants `01–04`, tint only on hover or drop |
| **Command-first, keyboard hints** (`⌘K`, `<kbd>`) | Linear, Akiflow | A `<kbd>` style for any shortcuts Iter adds later |
| **Empty states with personality** | Things, Sunsama | One line of friendly copy plus an icon, not "No items" |

## Tone

Sunsama's "Start calm. Stay focused. End confident." and Amie's playful
copy show that **calm** is the aesthetic of scheduling tools. Muted earthy
or near-neutral palettes, and color only where it means something.
