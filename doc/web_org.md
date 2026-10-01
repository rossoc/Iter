# Organization page redesign (`/org/{id}`)

> Superseded (final cleanup, `doc/web_cleanup.md`): the compare switch and `?design=old` were removed; the pages render the current design only. References below to the old design, the switch or `proposal` (now `screen`) describe how it was built, not how it is.


Scope: the organization page (both tabs). The edit form stays for later.
It builds on the finished homepage (`doc/web_homepage.md`).

## Plan

### Shared groundwork (so every page after this reuses it)
1. New `src/web/v2.rs`: the proposal's frame (top bar and centered `main`), the icons, `tilde`, and the compare switch. `home.rs` uses it instead of its own copies.
2. New `src/web/v2.css`, served at `/v2.css`, holding the tokens, both themes, the top bar, links with the sliding line, focus, labels, the compare switch and reduced motion. `home.css` keeps only the home-only rules (card grid, empty state). Rules stay scoped under `.v2` / `.compare`.
3. The compare switch keeps the current tab: on `/org/1?tab=tasks` it links to `?tab=tasks&design=old` and back.

### Comparing
4. `/org/{id}` shows the proposal. `?design=old` shows the current page, unchanged apart from the switch. Known limit: the old page's own tab links drop `design=old`, so clicking a tab there returns to the proposal.

### Proposal
5. Header: an "Organization" eyebrow, the name as `h1`, and an **Edit** button with a pencil icon on the right. The pencil icon is new to the icon set.
6. Tabs: **Info**, **Tasks** (with a count) and **Report**. The count comes from one `COUNT` query; the task list is loaded only on the Tasks tab. The active tab has an accent underline.
7. Info tab, in two columns (one below 900px):
   - Left: the description in a readable column with its line breaks kept, then **Projects** as rows (name, then its path in mono), like the homepage cards.
   - Right: a **Settings** panel. Yes/no values show as an "on" (check icon, accent) or "off" (muted) marker; text values in mono, with "—" when empty.
8. Tasks tab: a table with Task, Project, Status and Issue. Hairline rows with a hover tint. The status is a glyph pill: ○ queued (muted), ◐ in progress (accent), ✓ done (green: Tokyo Night `#9ece6a` in dark mode). The issue is mono `#12`.
9. Empty states: "No projects." and "No tasks.", the same text as today.
10. Accessibility as on the homepage: tabs use `aria-current`, status glyphs are `aria-hidden` with the word next to them, the table has a caption for screen readers, and the whole page is checked with axe.

### Feature changes
- Changed: the task count is shown on the Tasks tab (item 6).
- Added: the compare switch (item 4).
- Removed: nothing.

## What changed, checked against the plan

| # | Plan | Status | Where |
|---|---|---|---|
| 1 | Shared `v2.rs` (frame, icons, `tilde`, switch); `home.rs` uses it | ✅ Home re-screenshotted: unchanged | `src/web/v2.rs`, `home.rs` |
| 2 | Shared `v2.css`; `home.css` only home rules | ✅ Everything is still scoped (`.v2` / `.compare`) | `src/web/v2.css`, `home.css` |
| 3 | The switch keeps the tab | ✅ `?tab=tasks` ↔ `?tab=tasks&design=old` checked by clicking | `v2::compare`, `v2::old_url` |
| 4 | Proposal by default, `?design=old` the current page | ✅ The old tab links still drop `design=old` (known limit) | `pages.rs` `org_page` |
| 5 | Eyebrow, `h1`, Edit button with pencil | ✅ | `org.rs` `proposal` |
| 6 | Info / Tasks tabs, count on Tasks | ✅ Tasks are loaded on both tabs in the proposal (+0.09 ms) | `org.rs`, `pages.rs` |
| 7 | Two-column Info: description, project rows, Settings panel | ✅ The description keeps its line breaks | `org.rs` `info`, `settings_panel` |
| 8 | Tasks table with status pills and mono issue | ✅ | `org.rs` `task_table` |
| 9 | Empty states | ✅ | `org.rs` |
| 10 | Accessibility | ✅ axe: 0 violations on both tabs, light and dark, at 1280px and 320px | |

Not in the plan (found while testing):
- In light mode, "off" and "—" in the Settings panel failed contrast on the panel's darker background, so they use `--muted` instead of `--faint`.
- At 320px both tabs scrolled sideways. Now the Info column can shrink (`minmax(0,1fr)`), a long name wraps with Edit dropping below it, and below 600px each task row stacks (name, then project, status and issue). Checked at 320, 480, 700 and 1280px: no horizontal scroll.
- New shared pieces in `v2.css`: the `--ok` green token (light `#15803d`, dark Tokyo Night `#9ece6a`), `.mono`, and `.sr` (screen-reader-only text). New icons in `v2.rs`: check, minus, circle, a half circle (drawn here, not Lucide) and circle-check.

Side effects on the old design: it loads `/v2.css` through the switch, and the switch is shown. Nothing else changes. axe still flags the old page's own issues (a low-contrast selected sidebar item, `h1`→`h3`, small link targets). Those are left as the reference.

Verified: `cargo fmt --check`, clippy (0 warnings), and 176 tests pass. With the release build, `/org/1` takes 0.45 ms at the median on both tabs (old design: 0.36 / 0.44 ms).

## Rev 1: Report tab — plan

The report that `iter org info` prints, as a third tab: `/org/{id}?tab=report`.

1. **Same numbers as the CLI:** built by `utils::report::project_reports` (the same function `iter org info` uses), so totals merge overlapping sessions and short pauses exactly like the terminal report. Only projects and tasks with sessions in the period appear, as in the CLI.
2. **Period:** preset links **Today · 7 days · 30 days · All time**, plus a From/To date form (`?from=YYYY-MM-DD&to=YYYY-MM-DD`, a plain GET form, no JS). The default is **7 days** (the CLI defaults to today). An invalid date shows an error and falls back to the default.
3. **Summary:** the total time, then "N projects · N tasks · N sessions" and the period.
4. **Contents (TOC):** one entry per project with its time and a bar showing its share. Clicking one jumps to that project's section. Sticky beside the sections on wide screens, above them below 900px.
5. **Project sections** (`#p-{id}`): the name (links to the project) and its total. Then each task: its name (links to the task), a status pill, its time, its description if any, and a session table (date when the period spans several days, start, end, duration, message). Each section ends with a "Back to contents" link.
6. **Navigation polish:** smooth scrolling (off under reduced motion), sections stop below the sticky top bar, and the section you jumped to briefly highlights.
7. **Code:** `ProjectReport` and `TaskReport` get an `id` field with `#[serde(skip)]` so the page can link to them. The CLI's YAML and text output are unchanged.
8. **Old design:** has no Report tab. `?tab=report&design=old` shows the old Info tab.
9. **Feature changes:** added the Report tab. Nothing removed.

## Rev 1: Report tab — checked against the plan

| # | Plan | Status | Where |
|---|---|---|---|
| 1 | Same numbers as `iter org info` | ✅ 7-day totals compared project by project with the CLI's YAML: identical. An open session made the two runs differ by one minute | `org::load_report` → `utils::report::project_reports` |
| 2 | Presets, From/To form, default 7 days, bad input | ✅ All four presets and the form clicked through; `from=bogus` and a reversed range both show a message and fall back to 7 days | `org::period`, `period_picker` |
| 3 | Summary line | ✅ | `report_view` |
| 4 | Contents with time and share bars, sticky | ✅ Clicking an entry lands the section 20px below the top bar; the contents stay visible beside it | `report_view`, `.toc` |
| 5 | Project and task sections, session tables, links, back link | ✅ Project and task links and "Back to contents" all checked | `task_section` |
| 6 | Smooth scroll, scroll margin, highlight | ✅ All three are off under reduced motion | `org.css` |
| 7 | `id` on `ProjectReport` / `TaskReport`, `#[serde(skip)]` | ✅ The 176 tests, including the CLI's report rendering tests, still pass | `reporting.rs`, `utils/report.rs` |
| 8 | The old design has no Report tab | ✅ It shows Info, and the switch returns to the same report | `pages.rs` `org_page` |
| 9 | Only the Report tab added | ✅ | |

Not in the plan (found while testing):
- `Total` also got a `#[serde(skip)] minutes` field. The share bars need exact minutes, and `Total` only kept rounded hours plus a string.
- The TOC bar class `bar` clashed with the top bar's `.bar` and picked up its padding. It's now `.share`.
- The switch lost the report on the way back (Before → Proposal landed on Info). Its link is now built from the query rather than from the tab being shown.
- The preset pills wrapped mid-label at phone width. They now stay on one line and get tighter below 600px.
- On phones, session rows stack like the task rows (times on one line, the note below).

Verified: axe reports 0 violations (light and dark, at 1280 and 320px), with no horizontal scroll at either width. `cargo fmt --check`, clippy (0 warnings) and 176 tests pass. Release build: the 7-day report takes 0.62 ms (13 KB) and the all-time report 0.81 ms (25 KB, 82 sessions).

Seen in the data, not a code issue: some sessions end the day after they start (e.g. 26:28), and one has been open since 2026-09-22, so it counts up to now. The report shows them as recorded. An open session's end reads "now".

## Rev 2: report trimmed

| Change | Done |
|---|---|
| "Back to contents" removed | ✅ Link and `.top` CSS deleted |
| Sessions show Date, Start, Duration (no End) | ✅ A still-open session gets a small breathing accent dot after its duration, with "(still running)" for screen readers, since there's no End column to say "now" any more. The dot holds still under reduced motion |
| The summary doesn't repeat the period or the hours | ✅ Only the big total and "N projects · N tasks · N sessions"; the dates are already in the From/To fields and the preset |
| From, To and the fields aligned | ✅ Measured: label text and inputs centered within 0.5px of each other |
| No Show button; the report follows the dates | ✅ A small inline script (`REPORT_JS` in `org.rs`) |

How dates apply (checked with playwright):
- Picked from the calendar: applies at once.
- Typed: applies on Enter, or when focus leaves both date fields. Not per keystroke: Chrome reports a change after every digit of a year (`0002`, `0020`, …), which reloaded mid-typing in the first attempt. Only a complete four-digit year from 1970 counts.
- Without JavaScript: Enter still submits, through a visually hidden submit button. The button is out of the tab order, since keyboard users reach it through Enter.

Verified: axe clean in light and dark. `cargo fmt --check`, clippy (0 warnings) and 176 tests pass.

## Rev 3: no sub-minute sessions; aligned period row

Sessions under a minute are never stored, and the ones already stored are deleted. This is a CLI change, not only a web one:
- `Session::MIN_SECONDS = 60` and `Session::too_short` (`models/session.rs`).
- `utils::clock::close_open_session` deletes an open session instead of closing it when it ran under a minute, and reports `Closed::Discarded`. This covers `iter session stop`, `iter task done` and the tmux detach hook. `iter session stop` then prints "discarded the session (under a minute) for '…'".
- A third migration, `Db::drop_short_sessions`, deletes closed sessions under 60s. It measures whole seconds with `strftime('%s')`, because `julianday` float error puts an exact minute at 59.99…. Open sessions are never touched, and a legacy table without `start`/`end` is skipped.
- Tests: 59s is discarded, exactly 60s is kept, closing with nothing open changes nothing, and the migration keeps the 60s and open rows. 180 tests pass.
- On a snapshot of the real database: 181 → 105 sessions (the 76 under a minute, one of them an 11s test note "a new message"). All 7 open sessions kept; schema version 2 → 3. The real database migrates the next time `iter` opens it.

Period row: the preset pills and the date fields share `--control-h: 36px` and one center line. The first measurement was 4px off: `style.css`'s global `form label { margin: 12px 0 4px }` leaked into the new form, so `.v2 .range label` now sets `margin: 0`. Measured at 1280 and 1000px in light and dark: pills, fields and row are all 36px on the same center, with the label text within 0.5px. axe is still clean.

## Rev 4: tabs share one sliding underline

Info / Tasks / Report now have a single underline. It rests under the current tab. Hovering or keyboard-focusing another tab moves it there: the current line retracts to the right while the new one grows from the left. This is the same motion as the links' sliding line (2px `--line-w`, 350ms, `--ease-out`). The text color moves with it. Pure CSS: `.tabs:has(a:hover, a:focus-visible)` releases the current tab's line.

Checked by sampling each tab's `::after` scale with playwright:
- At rest, only the current tab has a line.
- 90ms into hovering Info, Info is at 0.75 (growing from the left) and Report at 0.25 (retracting to the right). Settled, only the hovered tab.
- Leaving the tabs slides the line back. Hovering the current tab changes nothing. Keyboard focus behaves like hover.
- Instant under reduced motion. axe clean on both tabs in light and dark.

## Rev 5: period pills share one highlight

Today / 7 days / 30 days / All time work like the tabs. One pill is highlighted: the current period at rest, otherwise the hovered or keyboard-focused one. The old highlight fades and shrinks away (to 80%) while the new one grows in (250ms fade, 350ms scale, `--ease-out`), and leaving the pills returns it to the current period. The text color moves with it. Pure CSS, the same `:has(a:hover, a:focus-visible)` release as the tabs. The highlight is a `::before` layer behind each pill, so the pill's text never moves.

Checked by sampling each pill's highlight opacity with playwright:
- At rest, only the current period.
- 80ms into hovering 30 days, 30 days is at 0.87 and 7 days at 0.13. Settled, only the hovered pill. Moving across Today and All time moves it along, and leaving returns it to 7 days.
- Hovering the current pill changes nothing. Keyboard focus behaves like hover.
- Instant under reduced motion (transition 0s). axe clean in light and dark while hovering.

## Rev 6: darker form fields; Copy summary

- Form fields (`input`, `select`, `textarea`) sit on `--bg-2`, one step darker than the page (dark `#1f2335` on `#24283b`). The rule is in `v2.css`, so every redesigned form gets it.
- The report's summary row ends with **Copy summary** (first named "Copy statuses"). It copies the period's projects and their tasks' statuses:
  ```
  BrickAdapter:
  - Missing-fields: done
  NEM:
  - NN-implementation: work in progress
  ```
  The text is built on the server (`org::status_list`) from the same report data as the page, so it covers exactly the projects and tasks shown for the period. It uses the same status wording as the pills (queued / work in progress / done). The button shows "Copied" with a check for 2 seconds and announces it to screen readers (`role="status"`). It stays hidden without JavaScript or a clipboard, and isn't rendered when the period has no sessions. Its margin, padding and colors are all set explicitly, because `style.css` styles every `button`.

Checked with playwright, with clipboard permission granted: clicking copies text identical to the page's projects and tasks, the feedback shows and clears, the button is hidden with JavaScript off and absent for an empty period. axe clean in light and dark; no horizontal scroll at 320px. 180 tests pass.

## Rev 7: date fields light up what a click selects

The From/To fields (shared rules in `v2.css`, for every date field in the proposal):
- Hovering month, day or year turns only that part accent (orange in dark, blue in light).
- Hovering the calendar icon turns the whole date and the icon accent, since a click there picks a whole date. CSS can't reach from the icon to the text, so `REPORT_JS` sets `.whole` while the pointer is over the icon's 14px, plus its 8px margin. The icon is a Lucide calendar drawn as a CSS mask, so it can change color and has a known size.
- Measured with pixel captures in dark and light. Month only lit x 13–25, day only 38–50, year only 64–92. The icon lit the date's digits, the separators and the icon (x 120–131). The field's right padding lit nothing.

Known limit: the part a click selects stays in Chrome's own blue. Chrome paints that highlight over author styles. Seven approaches all left it blue, including `:focus` with `!important`, `::selection`, `:focus-visible` and `accent-color`.

These are `::-webkit-` parts, so Chromium only. Firefox shows its own field unchanged, and the script's class is harmless there. axe clean in light and dark; picking a date still applies it at once.

## Rev 8: project rows highlight on hover

The Info tab's project rows now highlight like the Tasks table's rows: the same `--bg-2` tint, the same 150ms fade, and the same inset (text 4px from the left edge, 14px on the right). On a hovered row the path lifts from `--faint` to `--muted`, because `--faint` is only 4.33:1 on the tint in light mode (axe caught it); `--muted` is 4.74:1. Checked with playwright: the tint matches the task rows in both themes, only the hovered row changes, and axe is clean with each of the six rows hovered, in light and dark.

## Update: task search, quick add, project search and create row

- **Tasks tab**: a search box (`?tab=tasks&q=`) over the list, parsed by `web/task_query.rs`: `is:open` (default, pre-filled), `is:done|wip|queue|all`, `tag:name` / `tag:"two words"`, `project:name`, `-term` to negate, anything else a case-insensitive substring of the task name (words ANDed; `#12` is the issue). Unknown `key:value` terms are name text, never an error. The browser sends an empty `q=` as no `q`, so an empty box gives the default; `is:all` shows everything. The tab badge keeps the total; "N of M tasks match" shows when filtered. Tags are read in one query (`Db::tag_names_for_tasks`) only when the query has a `tag:` term.
- **Quick add**: the table's first row is a one-field form (`ui::task_table::QuickAdd`; the controls name an empty `<form id="quick-task">` by `form=`) posting to `POST /task/quick` (`task_new.rs`): the task is built as the New task form builds it (project template, `TaskForm` validation), and on success the browser returns to the same list, keeping the search. A refusal shows the full New task screen with the error and the typed name. On an organization the row has a project `<select>`; the full "New task" button stays on the project page.
- **Info tab**: the project list has a search (`?q=`, name or base path, words ANDed) and, on the organization only, a create row (name, base path, Add project, Browse folders). It posts to `POST /org/{id}/project/quick` (`project_new.rs`): only the DB row is inserted (no scaffolding), from the config's project defaults with the organization's settings over them, with `ProjectForm` validation; it returns to the organization, or shows the page again with the error and what was typed. A board's Info has the same row at `POST /board/{id}/project/quick`: the project has no organization and sits on the board. **Browse folders** (`/folders`, `folders.rs`) is a GET page carrying the typed name, the path and `home` (`org:3` / `board:2`): it lists subfolders as links (into, up, show hidden), and "Use this folder" (with an editable name, default the folder's name) posts the folder as the base path. No script.

## Update: New task / New project pop-ups

The quick-add row and the inline project create row are replaced by a clickable first row ("+ New task…", "+ New project…") that opens a pop-up with the whole form (`ui/modal`, no script: `?new=task|project`, the page behind `inert`). New task posts to `POST /task/new` and asks for the project on an organization; New project has description, base path (Browse folders) and the Settings group. The folder browser returns to the pop-up instead of creating the project. See `doc/web_components.md`, "Pop-ups and the "?" syntax help".
