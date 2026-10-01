# Accessibility review (WCAG 2.2 AA)

Branch `feat/html-refactor`, commit 146aeae. Reviewer lens: accessibility only. No source was edited.

## Scope

- `src/web/**` (pages, `ui/*.rs`, `ui/*.css`, `v2.css`, `org.css`), `board.js`, `org.js`, `assets.rs`, `layout.rs`, `errors.rs`, the drop endpoints `drop.rs` and `agenda_drop.rs`, and `design/` (only for intent).
- Pages rendered and tested: home, boards, board agenda, agenda pick mode, matrix, matrix pick mode, board info, board edit, org info/edit/report/tasks, org "New project" modal (also its refused-form response), project info/edit, task page/edit, settings, 404.

## Method

Commands and tools used:

- Built binary `target/debug/Iter` (newer than every `src` file, checked with `find -newer`). Ran `Iter serve --port 8765` against a throwaway database under `XDG_CONFIG_HOME=<scratchpad>/xdg`. I seeded it with `sqlite3` (1 org, 1 project, 1 board, 2 tasks, urgent/important tags). The real database was not touched. Server stopped afterwards, `git status` clean apart from this file.
- `curl` to fetch 24 URLs, and a small Python `html.parser` audit for heading order, duplicate ids, `label for` targets, landmarks, `<title>` count, images.
- Headless Google Chrome via `puppeteer-core` with `axe-core` 4.x (tags wcag2a/aa, wcag21a/aa, wcag22aa, best-practice) on 17 pages in light and dark `prefers-color-scheme` at 1280 px.
- Puppeteer scripts for: horizontal overflow at 320 px and 400 px wide; target sizes (below 24 px) at 320 px; real Tab walks (modal, agenda, `focus`/outline state); `elementFromPoint` check that no focused element is covered by the sticky bar at 1280x800, 1280x620, 1000x700; Chrome accessibility tree snapshot of the agenda; keyboard ArrowUp on the date field; Escape on the search-syntax tip and on the modal.
- Contrast ratios computed with a Python WCAG luminance function over every token in `v2.css:21-45` against `--bg`, `--bg-2`, `--bg-3` and the accent tint (`color-mix` approximated in sRGB, so +/-0.1).
- Files read: `v2.css`, `board.js`, `org.js`, `drop.rs`, `agenda_drop.rs`, `layout.rs`, `assets.rs`, `errors.rs`, `ui/{field,form_error,notice,modal,filter_bar,frame,pick_bar,side_rail,folded_list,day_nav,jump_link,task_card,tag_chips,ink,breadcrumb}.rs`, and most of the matching `ui/*.css` plus `org.css`.
- Not checked: other `doc/review_*.md` (as instructed); `doc/web_*.md` (context only, not read in depth).

What I did NOT check: see the last section.

### Verdict by area (what is already good)

Verified by axe (0 violations on 17 pages x 2 schemes, except F1) and by reading:

- `lang="en"` (`layout.rs:21`), viewport meta without zoom lock, one `<h1>` per page, no skipped heading levels (h1 then h2 only), no duplicate ids, every control has a `label for` (`ui/field.rs:56-77`) or an implicit label, no images (all icons are `aria-hidden` SVG).
- Landmarks: skip link first, `header`, labelled `nav`s, `main#main tabindex=-1`, labelled `aside`, `footer`, `role=search`.
- Focus indicator: global `:focus-visible` 2 px accent ring (`v2.css:56`), `.control:focus` ring (`ui/control.css:10`). Every `outline:none` found (`task_card.css:27`, `project_list.css:36`, `task_table.css:19`, `side_lists.css:9`, `filter_bar.css:24`, `frame.css:27`) has a replacement or is a programmatic-focus target only. The sticky bar never covers a focused element (checked by script).
- Drag and drop has a complete keyboard/pointer alternative: every card carries a "Move/Schedule/Place" link into pick mode, with "Schedule here . HH:00" buttons per hour (`ui/pick_here.css`, `ui/pick_bar.rs`). Dragging is never required (2.5.7 met). Failed drops are announced with `role=alert` (`board.js:81-88`). Successful drops reload onto the moved card with a status note (`board.js:57-67,125-136`).
- Errors (3.3.1/3.3.3): `error_box` focuses an `id=form-error` notice (verified: `document.activeElement` is the notice), has a visible "Error:" text, links to the field, sets `aria-invalid` and `aria-describedby`, adds a non-colour "Fix this" cue and a thicker left border (`ui/notice.rs`, `ui/field.rs`, `notice.css:9`).
- Reduced motion: every transform/animation is gated behind `prefers-reduced-motion:no-preference` or zeroed (`v2.css:80`, `org.css:57,68-69`, `modal.css:16-19`, `sessions_table.css:13`); the pulsing dot stops after 3 cycles.
- Forced-colors fallbacks are provided on tabs, nav, cards, drops, notices.
- Reflow: no horizontal overflow at 320 px or 400 px on any of the 17 pages (verified); sticky parts switch off under 900 px wide or 600 px high.
- Colour: all text tokens pass AA on their intended surfaces (table below). Tag chip ink is computed to >= 4.5:1 and unit-tested (`ui/ink.rs`).

Contrast table (computed):

| Token pair | Light | Dark |
|---|---|---|
| `--fg` on bg / bg-2 / bg-3 | 16.3 / 15.2 / 13.9 | 6.9 / 7.4 / 6.4 |
| `--muted` on bg / bg-2 / bg-3 | 5.08 / 4.74 / **4.33** | 5.15 / 5.50 / 4.74 |
| `--faint` (decoration only) on bg / bg-2 | 4.64 / 4.33 | 4.60 / 4.91 |
| `--accent` on bg / bg-2 / accent-tint | 5.43 / 5.07 / ~4.6 | 7.16 / 7.65 / ~5.7 |
| `--ok`, `--danger` on bg | 5.33, 6.30 | 7.97, 5.51 |
| `--on-accent` on `--accent` | 5.67 | 7.16 |
| `--control-line` on bg / bg-2 / bg-3 | 3.80 / 3.55 / 3.24 | 3.49 / 3.73 / 3.22 |
| `--muted` on accent-tint | **4.30** | **4.11** |
| default Urgent edge `#facc15` on bg-2 | **1.37** | 10.2 |
| default Important edge `#3b82f6` on bg-2 | 3.29 | 4.23 |

## Findings

Severity: High = blocks users or fails a Level A/AA criterion with real impact; Medium = fails a criterion in a narrower case or degrades AT use clearly; Low = minor, best practice, or latent risk.

### F1 [Medium] 1.4.3 Contrast: inline `code` in muted text on `--bg-3` is 4.33:1 (light)
- `v2.css:66` gives `code` background `--bg-3` but no colour, so it inherits `--muted` from its parent. axe flagged it on `/settings`: `.foot-note code` at `settings.rs:177` (fg `#6f6b64` on `#ebe8e2`, 8.9 pt). Same trap exists wherever `code` sits in muted text; `ui/empty_state.css:6` already patches one case, and the comment at `task_card.css:35` shows the authors know about this exact pair.
- Fix: put the colour in the one rule, `.v2 code{color:var(--fg)}` at `v2.css:66`, and drop the per-component patches. Alternatively raise light `--muted` to about `#67635c` (>= 4.5 on bg-3).
- Verified: axe (the only violation in 34 runs); ratio computed.

### F2 [Medium] 3.2.2 On Input: picking or arrowing a date on the agenda reloads the page
- `board.js:41-43` submits the form on every `change` of `#day-input`. Verified in Chrome: focus the date field, press ArrowUp, and the browser navigates immediately to `?date=2026-10-02`. A keyboard or screen-reader user trying to edit the day segment by segment is thrown to another page after the first keystroke, with no warning.
- `org.js:11-24` already solves this correctly for the report dates (defers to Enter/blur when the user typed or used arrows). The agenda does not use it.
- Fix: reuse the `org.js` pattern (ignore `change` while a key is held/typing, apply on Enter or blur), or only auto-submit when the change came from the picker (`input` event after `showPicker()`), and otherwise rely on the hidden "Go to this date" button.
- Related, same area: the date field is a 1 px, `opacity:0` element (`day_nav.css:16`). While it is focused the label shows a ring (`day_nav.css:12`), but the edited value is invisible, and Chrome's accessibility tree exposes it as a button named "Show date picker Show date picker" (duplicated) inside the heading. Verified by snapshot. Consider a visible control, or at least hide the whole thing from non-script users' tab order cleanly.

### F3 [Medium] 2.4.3 / 2.4.7: modal does not contain focus; footer is reachable behind it
- `ui/frame.rs:60,68` makes only `header.bar` and `main` `inert` while a dialog is open. The footer is rendered by `layout.rs:29` (`<body>(slot) site_footer_for()</body>`), outside the frame, so it stays focusable and in the AT tree. Verified: in the "New project" modal, Tab from "Cancel" lands on footer links (`.foot-mark`, "Acme", "Alpha"), i.e. focus goes to controls hidden behind the backdrop. The skip link (`frame.rs:58`) is also outside the inert parts.
- Related: Escape does nothing (verified; `modal.rs:1-6` admits "no script means no Esc"). Not a WCAG failure because a visible Close link exists, but `role=dialog aria-modal` implies Esc in every AT user's model, and `board.js` is not loaded on org/project pages anyway.
- Fix: mark the footer and skip link inert too (e.g. pass `covered` from `frame` to a wrapper, or emit `inert` on `.site-foot` when a `.modal` exists via `body:has(.modal) .site-foot` plus an `inert` attribute written by the layout), or wrap bar+main+footer in one inert container. A 10-line script (focus trap + Escape follows the Close link) would be a progressive enhancement.
- Verified by running.

### F4 [Medium] 1.4.1 Use of colour / 1.4.11 non-text contrast: task priority is shown to sighted users by colour only
- A card's Urgent/Important state is a 4 px coloured edge (`ui/edge.css`, `ui/task_card.css:5-8`). The words ("Urgent", "Urgent, important") are `class="sr"`, visible only to screen readers (`ui/task_card.rs:105-107`). The comment at `task_card.css:2-3` says "the flags are also chips with words", but no chip is rendered (checked in the rendered HTML of `/board/1` and `/board/1/matrix`).
- The colours are user-configurable tag colours; default Urgent yellow is 1.37:1 against the light card surface (computed), well under 3:1. On the agenda, where position does not encode priority, a sighted user with colour-vision deficiency or low vision cannot tell urgent from other cards. (The matrix encodes it by quadrant, so it is less severe there.)
- Fix: render a visible text chip ("Urgent", "Important") in `.tcard-meta`, or a letter/icon with text, and keep the edge as a redundant cue.
- Inferred from code and rendered HTML; contrast computed.

### F5 [Medium] 1.3.1 Headings inside `<summary>` are not exposed as headings
- `ui/folded_list.rs:24` and `ui/unplaced_list.rs:36` put `<h2>` inside `<summary>`. In Chrome's accessibility tree (puppeteer snapshot on `/board/1`) the three "Important/Urgent/Backlog, not scheduled" sections appear as regions with names but no heading nodes; the other `h2`s (day title, footer) do. Heading-navigation users miss these sections. Summary content is treated as presentational in some engines.
- Also the heading is the toggle's whole label, including the count badge text "(1 task)".
- Fix: put the heading outside and make the `<summary>` the button (`<h2><details>...` is not valid), or drop `<details>` for a `button[aria-expanded]` pattern inside the `<h2>`; or accept the section `aria-labelledby` as the nav route and document it.
- Verified by Chrome accessibility snapshot; other AT may differ.

### F6 [Low-Medium] 1.4.13 Content on hover or focus: search-syntax tip cannot be dismissed with Esc
- `ui/filter_bar.css:21-26` shows `.filter-tip` on `:hover` and `:focus-within`; pressing Escape does nothing (verified: `display` stays `block`). It is hoverable and persistent, so two of three conditions hold; dismissibility fails. The tip has `role="tooltip"` but contains a `<dl>` of interactive-looking content (`ui/filter_bar.rs:58-64`).
- Fix: a tiny `keydown` handler (or `:has(:focus)`-independent approach) that blurs/closes on Escape; or make it a `<details>` disclosure that stays open by user action.

### F7 [Low-Medium] 4.1.2 / valid HTML: `<title>` and `<link>` are emitted inside `<body>`
- `ui/frame.rs:53-57` writes `<title>` and the stylesheet links as the first body children; the layout `<head>` (`layout.rs:22-25`) has only charset and viewport. Browsers and AT currently take the title, but this is invalid HTML, causes a flash of unstyled content (stylesheet discovered in body), and some crawlers/validators/AT may not take it. The comment explains it is a framework limit.
- Verified in rendered HTML (`curl /`).
- Fix: if the framework allows head injection, move title and CSS there; otherwise accept and track.

### F8 [Low] 3.3.1 Refused modal forms do not say "Error:" in the document title
- `ui/frame.rs:30` supports `error: true` ("Error: ..."), but `org.rs:187-191` (and the board/project modal hosts) do not pass it for the refused "New project/New task" modal. Rendered response to a bad `POST /org/1/project/quick` has `<title>Organization - Acme - iter</title>`. The focused notice mitigates this for AT, so impact is small.
- Verified by `curl`.

### F9 [Low] 2.5.8 Target size: several controls are below 24 px but pass only by the spacing exception
- Measured at 320 px: card title links 20 px tall (`.tcard-title`), checkbox inputs 16x16 (their labels have 32 px rows, `field.css:14`, so fine), breadcrumb links ~13 px tall (`breadcrumb.css`, `.label` is .6875rem), footer sub-links ~17-20 px. None overlap neighbours' 24 px circles, so they pass 2.5.8, but the margin is thin (3 px, `site_footer.css:19`).
- The hidden Apply button (22x8) is only shown on focus and is not a concern.
- Fix (optional): give `.tcard-title` and `.crumbs a` `padding-block:2px`+ or `display:inline-block;min-height:24px`.
- Verified by measurement script.

### F10 [Low] Hover-only visibility of the card action on desktop
- `ui/task_card.css:21-24`: with a fine pointer and >= 901 px the "Move/Schedule/Place" link is `opacity:0` until hover or focus-within. It is reachable by keyboard and revealed on focus, so 2.1.1/2.4.7 are met. But voice-control users ("click Move") and screen-magnifier users have no visible cue that the command exists, and it is the page's only non-drag way to move a card.
- Fix: keep it visible at a low-emphasis style (muted underline) at all sizes.
- Inferred.

### F11 [Low] Success note after a drop depends on a timing hack
- `board.js:48-55` clears the note text and rewrites it after 100 ms so the live region announces it. This is a known workaround, but a slow device/AT may miss it. After `location.replace` to `#card-N` the focus moves to the card (`tabindex=-1`, no visible ring: `task_card.css:27`), which is intentional but means the keyboard user's position is not visually shown after a pick-mode move.
- Fix: give the focused card `:focus-visible` a ring (the card has `tabindex=-1`, so only programmatic focus triggers it) rather than `outline:none`.
- Inferred; not tested with a screen reader.

### F12 [Low] Light-theme accent on accent-tint is borderline
- `ui/status.css:3` (WIP chip), `day_nav.css:22` (Today), `frame.css:11` (nav current) use `--accent` on `--accent-tint`: about 4.6:1 in light (my sRGB approximation of the `oklab` mix; the browser's value may differ by 0.1). axe did not flag it. A slightly darker light `--accent` (e.g. `#3451c8`) gives margin. `--muted` on the tint is 4.30 (light) / 4.11 (dark); axe found no instance, but `.bar nav a` and `.presets a` can show `--muted` on the tint during hover transitions.

### F13 [Low] Table semantics on narrow screens
- `sessions_table.css:19-30` and `task_table.css:23-30` switch `tr`/`td` to `display:flex/block` under 600 px and hide `thead` by clip. Some browsers drop table semantics when `display` is overridden, so the stacked rows lose headers for AT. Fix: add explicit `role="table|row|cell"` or keep the `display` and re-flow with grid on `tr` only.
- Inferred (browser-dependent; not tested with VoiceOver).

### F14 [Info] Nits that need no action now
- Nav uses `aria-current="true"` (`ui/frame.rs:63`); `"page"` is more specific.
- Footer `<h2>`s add four headings to every page's outline (`ui/site_footer.rs`). Harmless; contributes to long heading lists.
- `role="list"` on a `ul` (`ui/tag_chips.rs`) is a deliberate Safari workaround for `list-style:none`; fine.
- `content-visibility:auto` on the footer (`site_footer.css:3-4`) keeps contents in the AT tree; find-in-page and Tab still work (Tab reached footer links in the modal test).

## Confidence

- High: contrast numbers, axe results (0 violations apart from F1), heading/label/landmark structure, F2, F3, F6 (all reproduced in Chrome), reflow at 320/400 px, target-size measurements.
- Medium: F4 (code and rendered HTML confirm there is no visible text, but I did not view the screenshot with a CVD simulator), F5 (Chrome tree only), F12 (colour mixing approximated).
- Low: F10, F11, F13 (reasoned, not tested with AT).

## What I could not assess

- Real screen-reader behaviour (VoiceOver/NVDA/JAWS): live-region timing after a drop, how summary/heading and the hidden date field are announced, table semantics under 600 px.
- Safari and Firefox (only Chrome headless was available); `showPicker`, `inert`, `content-visibility` and date-input behaviour differ.
- Actual drag-and-drop gesture with a mouse/touch (HTML5 DnD does not run in headless puppeteer; I read `board.js`/`drop.rs`/`agenda_drop.rs`, and verified the pick-mode alternative renders and posts to the same endpoint). Touch devices have no DnD at all; they rely on the pick links, which look sufficient.
- Text-spacing override (1.4.12) and 200% browser text-only zoom: only viewport widths of 320/400 px were tested, not user-stylesheet spacing.
- Forced-colors / high-contrast rendering (only read the rules), print styles, and Windows contrast themes.
- Visual rendering of pages (no screenshots reviewed): focus-ring appearance and hover states were inferred from CSS.
- Pages needing data I did not seed (sessions with notes, tags page, `?new=task` flows, `/folders/pick`, board edit with many projects, long names/overflow cases).
- `design/` documents were only skimmed for intent; no accessibility claims in them were checked.
