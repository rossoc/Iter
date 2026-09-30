# Task page redesign (`/task/{id}`)

> Superseded (final cleanup, `doc/web_cleanup.md`): the compare switch and `?design=old` were removed; the pages render the current design only. References below to the old design, the switch or `proposal` (now `screen`) describe how it was built, not how it is.


Scope: the task page. The task edit form (page 7) is left for later and keeps working. It builds on Org (`doc/web_org.md`), Project (`doc/web_project.md`), the edit pages and the shared components in `src/web/ui/` (`doc/web_components.md`). The page is the sibling of Org and Project: same header, two-column body, Details panel where they have Settings, a table like the Org report's.

## Plan

### Page
1. Same frame as the sibling pages (`ui::frame`), document title "Task - {name} - iter". Header is `page_header` with the **breadcrumb as kicker** (`Organization / Project / Task`, built by `breadcrumb::trail`; the organization only when there is one, the project linked to its Tasks tab), `h1` the task name, the Edit button (`edit_button`) on the right. The page has no tabs, so the last crumb is `aria-current="page"` (`Crumb::here("Task")`).
2. Body in two columns (`ui::columns::info_columns`, one column below 900px):
   - Left: the description (`ui::prose::description`, nothing when empty), then **Sessions** (`labelled_section`, an `h2`).
   - Right: a **Details** panel (`ui::panel`) with the rows Status, GitHub issue, Branch prefix, Start, Duration, Tags (batch C: the panel and the edit form use one label per field; "Planned" was dropped, the edit form's Schedule group carries it). Reading order and heading order: `h1`, Sessions `h2`, Details `h2`.
3. Status is the shared glyph pill (`ui::status`), the issue is mono `#12`, unset values read "Not set" (`text_row`). Tags are colored chips.
4. Sessions is a table: Start, End (the word "ongoing" with the live dot for an open session), Duration, Note. No sessions: `empty_line("No sessions.")`.
5. No sidebar: only the old design loads it.

### New and generalized components (one feature each)
6. **`ui::sessions_table`** (`sessions_table.rs` and `.css`): a table of sessions. The Org report's table (`org.rs` `task_section`, its rules in `org.css`) is this table with different columns, so it is **generalized, not copied**: its markup moves to the component and its rules (`.sessions`, `.live`, the 600px stacking, the breathing dot) move from `org.css` to `sessions_table.css`. `sessions_table(lines: &[SessionLine], caption, #[default] show_end: bool)`. A `SessionLine` is display text: optional date, start and optional end as a `Moment` (text plus an ISO `datetime` for `<time>`), duration, note; no end means ongoing. Columns: Date (when any line has one), Start, End (`show_end`), Duration, Note. An ongoing line has the live dot: in the End cell (visible word "ongoing", the dot decorative) when the table has End, else next to the duration with the screen-reader text "(still running)" as the report has it. Tabular numbers (`font-variant-numeric`), mono, `<time datetime>` where the moment is known, `<caption class="sr">`, explicit roles (the narrow layout drops table semantics). Users: Org report (no End, no `datetime`), Task.
7. **`web/session_lines.rs`** (next to `task_rows.rs`): the mapping to `SessionLine`, one function per source: `report_lines` (the report's `reporting::SessionRow`) and `task_lines` (`Session`, measured against `now`; an end on the start's day shows only the time). The place that decides how a session reads.
8. **`ui::panel::value_row(label, child)`**: a Details/Settings row whose value is arbitrary content (the status pill, the chips). `flag_row` and `text_row` stay.
9. **`ui::tag_chips`** (`tag_chips.rs` and `.css`): `tag_chips(tags: &[Tag])`, a list (`ul`, explicit `list` role) of chips in the tags' colors. One function, **`readable_ink(color) -> Option<&'static str>`**: black or white, whichever contrasts more with the chip color by the WCAG formula; the better of the two is never under 4.58:1, so any `#rrggbb` gets at least 4.5:1. A value that is not `#rrggbb` (the database accepts any text) gets a neutral chip, and never reaches a `style` attribute. Tested: known colors, a sweep over the whole color cube for the 4.5:1 floor, malformed values.
10. Nothing else is new: `settings_panel` stays the Settings mapping; the Details rows are written in `web/task.rs`.

### Loading (`web/task.rs`, like `project.rs`)
11. `task::load(db, id)`: task, project, organization (only when the project has one), the task's tags, the task's sessions. Five point lookups by primary key or index (`idx_sessions_task_start`, `ORDER BY start` answered by the index, no sort in Rust), no loop over rows, no N+1. The route in `pages.rs` stays thin and adds `Nav::load` only for `?design=old`.

### Old code
12. `?design=old` on the GET keeps the previous page and sidebar with the compare switch. It still uses `pages.rs` `tabs`, `row`, `org_crumb`, `description`, `sessions_table` (renamed `old_sessions_table`, it is the old page's), so nothing there is deletable yet: the old project and org views use the same helpers, and `task_form` serves task edit (page 7). All of it goes with the final `?design=old` cleanup. The old `.chip` rule in `style.css` stays with the old page.

### Speed and accessibility
13. Still one stylesheet request (`/v2.css` gains two small sheets and loses the sessions rules from `org.css`).
14. Landmarks: header, breadcrumb `nav`, `main`, the panel `aside` with `aria-labelledby`. Details is a `dl`; the sessions table has a caption, `scope="col"` headers and explicit roles; at 320px the header row is visually hidden and each row is a flex row. Contrast at least 4.5:1 in both themes and for every chip; forced colors (the dot and chips get system colors); the breathing dot only when motion is allowed.

### Feature changes
- Added: the breadcrumb `Organization / Project / Task` with the last item current; the Details panel; an End column with "ongoing"; a "Not set" reading for empty values; tags as accessible chips; `<time>` elements.
- Changed: the chip text color follows the chip color (was fixed `#111`, which fails on dark tags); the sessions column headers are the report's ("Duration", "Note" for "Time", "Message"); the sidebar is gone; the Info/Edit tab row is now the header's Edit button.
- Removed: nothing (the old page stays under `?design=old`).

## What changed, checked against the plan

| # | Plan | Status | Where |
|---|---|---|---|
| 1 | Frame, breadcrumb kicker (last crumb current), Edit button, title "Task - {name} - iter" | done; project without organization shows `ITER / TASK` | `task.rs`, `ui::page_header` |
| 2 | Two columns: description and Sessions left, Details panel right; heading order h1, h2, h2 | done | `task.rs`, `ui::columns` |
| 3 | Status pill, mono issue, "Not set" | done | `ui::panel::text_row`, `value_row`, `ui::status` |
| 4 | Sessions table with End and "ongoing"; "No sessions." | done; an end on another day shows its date | `ui::sessions_table`, `session_lines.rs` |
| 5 | No sidebar | done | `pages.rs` |
| 6 | `sessions_table` generalized from the Org report | done; the report uses it (no End column), its rules moved from `org.css` | `ui/sessions_table.*`, `org.rs` |
| 7 | `session_lines` mapping | done, tested (same-day end, ongoing, midnight) | `session_lines.rs` |
| 8 | `value_row` | done | `ui/panel.rs` |
| 9 | `tag_chips`, `readable_ink` | done; a sweep test over the color cube proves 4.5:1; invalid colors get a neutral chip | `ui/tag_chips.*` |
| 11 | Five lookups, no N+1, sessions ordered by SQL index | done | `task::load` |
| 12 | `?design=old` | done; old helpers kept (still used by the other old views) | `pages.rs` |
| 13-14 | One stylesheet; accessibility | done | see below |

Not in the plan (found while testing):
- `--ok` (light) `#15803d` gave 4.48:1 on the panel's `--bg-2`, so the "done" pill failed axe there; now `#14783a` (4.97:1 on `--bg-2`, 5.3:1 on `--bg`).
- The Org report's task description had no `overflow-wrap`, so a long word scrolled the page sideways at 320px; added.
- The report view's future type needed `#![recursion_limit = "256"]` once `task_section` called one more component.
- The old `.chip` class in `style.css` would have leaked into v2, so the new chip is `.tag`.

Verified (throwaway copy of the database via `XDG_CONFIG_HOME`): a task with 9 sessions (one ongoing, one crossing midnight, a note with HTML) and 6 tags (dark, long name, invalid color `red`); tasks with no tags and no sessions, with and without an organization; 1280px and 320px, light and dark: axe 0 violations, no horizontal scroll; titles; heading order, landmarks, one `aria-current` (plus the temporary switch); keyboard order (skip link, brand, Board, crumbs, Edit, switch); forced colors and reduced motion (dot not animated); `?design=old` renders (its old axe issues predate this). Org report, Org, Project, project edit, org edit, new task: axe 0 and no scroll. Task edit (page 7) still renders (its old-form axe issues predate this). `cargo fmt --check`, clippy, 213 tests with `web`, 172 without.

## Review

Five reviewers; each finding checked before it was applied.

| Finding | Result |
|---|---|
| A1 sticky panel can trap content on short viewports | fixed: `.panel` is static under `(max-height:600px)` as well as under 900px wide (also Org/Project settings; they share the panel). Checked at 900x400: `position:static`, no clipping |
| A2 "still running" cue in the report | fixed: the `title` is gone (it double-read with the sr text); the dot, never dimmer than 0.6 opacity, plus the screen-reader text "(still running)" stays. The task page keeps the visible "ongoing" |
| A3 pulse | fixed: `animation-iteration-count:3`, still off under reduced motion |
| A4 mobile | `.note:empty` removal was fine. The arrow `content:"\2192" / ""` already has an empty alternative, so it is not read aloud: nothing to add (skipped). Notes keep line breaks: `white-space:pre-line` (with `overflow-wrap:anywhere`); notes are trimmed by `reporting::non_empty` |
| C1 `create_indexes` ignores errors | skipped: on purpose, per its doc (an index must not turn a slow read into no read, and an old schema may not take it yet); logging would need a logger the crate does not have |
| P1 `ui/` depends on `models::Tag` | fixed: `tag_chips(chips: &[Chip])`, `Chip { name, color }`; the mapping is `task_rows::chips`; colour logic is `ui/ink.rs` with its tests; `justify-content:flex-end` moved to `panel.css` (`.panel .chips`) |
| P2 `org_of` in `project.rs` | fixed: `web/load.rs`, used by project, task and new task |
| P3 `details()` in `task.rs` | fixed: `web/task_panel.rs`; `ui::panel::chips_row` renders "Not set" for no chips, so no branch (`text_row` and `chips_row` share one `unset` cell) |
| P4 `moment()` clash, stale docs | fixed: `session_lines::timed`; docs of `panel.rs` and `columns.rs` corrected |
| P5 other `ui/` components using models | fixed (small): breadcrumb `trail`/`edit_trail` moved to `web/crumbs.rs` (`Crumb` fields are plain); `project_list(items: &[ProjectItem])` with `web/project_rows.rs`. Left: `ui::status` takes the `TaskStatus` enum (a plain domain enum, not a row) |
| D1 duplicate `pages::issue` | fixed: `task_rows::issue_label` |
| D2 one end-of-span rule | fixed: `models::end_text(start, end)`, used by `session_lines` and `commands/board.rs` (CLI output unchanged, test added) |
| D3 duration text | fixed: `Session::duration_text(now)`, used by `session_lines` and `reporting::session_rows`; notes use `reporting::non_empty` (test added) |
| D4 old `.chip`, `.pill`, `old_task`, `old_sessions_table`, `stamp` | left: they go with the `?design=old` cleanup, after page 7 (task edit) no longer uses `task_form`, and page 12 |
| S1 chip shape | fixed with one deviation: padding `2px 10px` like the status pill, the border mix is commented; the radius is 14px, not `--r-pill`, because a long tag name that wraps turned into a blob with 999px |
| S2 "Start"/"Duration" wording | fixed: "Planned start", "Planned duration" (the slot the task is booked for, against the Sessions table's actual times). The table headers are unchanged. Page 7 keeps its edit labels "Start" and "Duration": they sit under the group "Schedule", which says the same; no rename needed |
| S3 Sessions count | fixed: `labelled_section(count: Option<usize>)` shows the badge (`count.css`, shared with the tabs). Order stays oldest first: the log reads downward and the open session is the last row |

Verified again (throwaway database via `XDG_CONFIG_HOME`; task with 9 sessions, one ongoing, one over midnight, a note with a line break and HTML, 6 tags including a long name and the invalid color `red`; tasks with no tags, no sessions, with and without an organization): 1280px and 320px, light and dark, and 900x400: axe 0 violations and no horizontal scroll on `/task/{id}`, the Org report, Org, Project, Project Tasks, Org edit, Project edit; keyboard order unchanged; forced colors; `?design=old` renders. `cargo fmt --check`, clippy, 217 tests with `web`, 174 without.
