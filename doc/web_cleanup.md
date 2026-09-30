# Final cleanup of `iter serve`

Scope: after the 12 pages of `doc/web.md` were redesigned, the old design and its comparison switch were removed. Routes, redirects and POST behaviour are unchanged.

## Removed
- The `?design=old` branch of every page, and the compare switch: `ui/compare.rs` (`compare`, `compared`, `DesignQuery`, `is_old`, `old_url`), `ui/compare.css`, the `design` query plumbing, `BoardLoad::Old` / `Loaded::Old` (`board_page.rs` now returns a plain `BoardPage`).
- `web/board.rs` (old agenda, matrix, info, edit, `Card::class`, old zones).
- `layout.rs`: `shell`, `Nav`, `Sel`, `cls`, `description`, `check`, `field`, `textarea`, `form_actions`, `old_project_checks`, the sidebar, the `/style.css` route. The layout is now just the document and `/board.js`.
- `pages.rs`: `sidebar`, `old_tab`, `tabs`, `row`, `settings`, `old_settings_fields`, `task_table`, `org_crumb`, `org_form`, `project_form`, `old_rows`, `old_sessions_table`, `old_task`, `task_form`, `stamp`, `yes_no`; `org::report_query_url` (only the switch used it).
- `style.css` (70 lines) and its `<link>`. What v2 pages still needed from it moved into `v2.css`: `* {box-sizing}` and `body {margin:0}` (with the page background).
- Comments about the old design, `border:0` on `.tabs a` (it only countered the old sheet).
- About 1,300 lines in all (`src/web` is 8,260 lines now).

## Changed
- `home.rs` loads `grouped::<Organization>` directly (no `Nav`); `boards`, `board_info`, `board_edit` load only what they draw.
- The pages' components were named `proposal`; they are `screen` now.
- One `path_param!(id)` in `web/mod.rs` (`super::Id`) instead of five copies.
- `.playwright-cli/` deleted and added to `.gitignore`.
- The `.v2` scoping prefix stays: rules are scoped under the `frame` wrapper, and dropping the prefix everywhere is a wide edit with no user-visible gain.

## Checked
- Lenses: accessibility (every page goes through `frame`: skip link, one `main`, one `h1`, titles, `aria-current` rules), queries (no N+1 in `src/web`; left: `utils::report::project_reports` runs one query per project of the organization, shared with `iter org info`), composability, DRY, tokens (no hard-coded colors in `ui/*.css`).
- Browser: every route at 1280px and 320px, light and dark: 200, axe 0 violations, no horizontal scroll, one `h1`, `?design=old` ignored; all four edit forms saved and refused, New task, drag and drop and keyboard pick on the agenda and the matrix.

## Component inventory
See `doc/web_components.md` (API of each component in `src/web/ui/`, one `.css` next to each `.rs`, concatenated into `/v2.css`; page assembly in `src/web/*.rs`).

## Batch A (verification and performance)

- **Error pages** (`errors.rs`): a layer turns topcoat's `NotFoundError` and `BadRequestError` into a page built like the others (title, skip link, one `main`, one `h1`, link home, `/v2.css`), for GET and HEAD only. POST and the drop script's `fetch` keep the raw status. A URL no route matches (`/nope`) is not an error the layer sees and keeps topcoat's bare 404.
- **Batching:** `utils::report::project_reports` reads all tasks and all sessions of an organization in two queries; the org report matches `iter org info` for the same period.
- **Caching** (`assets.rs`): `/v2.css`, `/org.css`, `/board.js` and the new `/org.js` had no cache headers. Each now carries an ETag (hash of the compiled-in text, computed once) and `Cache-Control: no-cache`; a matching `If-None-Match` gets a 304 with no body. The two inline report scripts moved to `org.js`, so the report page no longer carries them.
- **Blocking SQLite** (`blocking.rs`): handlers open the database and query it with blocking calls (up to 5 s of `busy_timeout`) on tokio workers. Rewriting each of the 13 handler sites for `spawn_blocking` would touch every page, and `Db` is not `Sync`. Instead one layer runs each request in `tokio::task::block_in_place` plus `Handle::block_on`, so the runtime moves the worker's other tasks away first. It needs the multi-threaded runtime, which `serve` builds. Measured with the write lock held and 40 blocked POSTs: `/v2.css` answered in 4.3 s before, 5 ms after. Follow-up if the server ever serves more than one person: a small connection pool and `spawn_blocking` around the data-loading half of each handler.
- **Not done:** `tasks_of` still maps project names with a `HashMap` and sorts in Rust (the SQL join needs the generic row mapper to take a joined column; the cost is a clone per task, negligible at this size, and covered by a test). `save_task` still rewrites tags on every edit (a delete and a few inserts of a handful of rows; a skip needs an extra read). Follow-up if either shows in a profile.
- **Unbounded lists:** the org report ("All time"), a task's sessions, the Tasks tabs and the board cards are not paginated. Sizes are personal-scale (dozens to hundreds of rows), the pages are server-rendered without per-row work, and pagination would break the report's copy summary and the boards' drag targets. Revisit if a page passes about 2,000 rows.
- The `.v2` scope is a leftover prefix, and 76px/72px in `org.css` are not tied to `--bar-h` (fixed in batch C: `.toc` uses `--stick-top`, `.proj` a calc from `--bar-h`).

## Batch B (structure and duplication; behaviour unchanged)

Every route, HTML output and POST stays identical: 25 URLs were fetched before and after (GET, attributes sorted, the time-dependent now line masked) with no difference, then the flows were run in a browser (see below).

- **P2**: page components are `screen` everywhere; the aliases in `pages.rs` went.
- **P1**: `ui/url.rs` is `web/url.rs`; `Section`, `TabQuery`, `section_tabs`, `tabs_for` are `web/sections.rs` (`ui/tabs.rs` keeps `Tab` and `tabs`, given ready hrefs); `first_line` is `web/notes.rs`; `tilde` is in `project_rows::project_items` (`ProjectItem.path` is display text). The one route string `ui` still repeats (the Board nav href in `ui/frame.rs`) is asserted equal to `url::BOARDS` by a test. `ui` imports only vocabulary types (`TaskStatus`, `Priority`, `is_hex_color`, `IterError`), and the docs say so.
- **P3**: `pages.rs` is deleted. `org`, `project`, `task`, `org_edit`, `project_edit`, `task_edit`, `task_new` own route, loader and `screen` (GET handler `show`, POST `save`); `layout::register` adds the layout and `/board.js`; `router()` folds every module's `register`. `org_report.rs` holds the Report tab (`org.rs` keeps `/org.css` and `/org.js`). The agenda is split: `agenda_drop.rs` (POST `/board/{id}/schedule`, `schedule`, `schedule_back`), `agenda_cards.rs` (`Cards`, `side`), `agenda.rs` (route, loader, a `Model` worked out before the component). `Cards` went to `agenda_cards.rs` and not `board_cards.rs` as the review said: it is agenda-only and builds the agenda's pick links, `board_cards.rs` is what both boards share. `forms.rs` is `forms/{mod,org,board,project,task}.rs`, tests moved with their form.
- **P4, D3**: `project_checklist(options, noun, error)` and `group_options::<G>` are shared by the organization and board edit pages; `org_in` is in `load.rs`; `task_new` is one `NewTask` (`load`, `values`, `parents`); board edit uses `edit::load_among`, which reads the boards once and takes the board out of them, so the page stays at two queries (`edit::load` would add a read by key). Skipped: an `EditForm` hook for base/action/cancel (three lines per page against one method per form); the second list read of a refused board POST (it feeds the "currently in" names).
- **P5**: the New task button in `ui/tasks_tab.rs` is one component; `pick::PickLinks` (`pick`, `cancel`, `moved`) replaces `pick_link`/`card_link`/`moved_link` and the agenda's `pick_url`/`cancel_url`/`moved_url`. Skipped: renaming `quadrants.rs`/`ui/quadrant.rs`, and splitting `ui/panel.rs` and `ui/field.rs` (one stylesheet each, not mechanical).
- **D1, D2, D4, D5**: `load::tasks_or_count`; `board_header::title_parts`; `pick::PickWords` per board; `web::now()`; `url.rs` uses `reporting::fmt_date`; named constants `DATETIME_ATTR` and `CLOCK_FMT`; the two duration formats are documented on `duration_label` (no `Duration::short`).
- **Clippy**: the loop in `quadrants.rs` test (this branch) and the item after the test module in `utils/clock.rs` (unchanged since `ada3cba`) are fixed. The no-default-features build warned about web-only fields and methods (`Total.minutes`, the two report `id` fields, `Db::tasks_in`, `count_tasks_in`, `list`, which predate this batch) and about the `is_hex_color` re-export (added in the uncommitted work): the fields and methods carry `#[cfg_attr(not(feature = "web"), allow(dead_code))]`, the re-export is gated on `web`. Both clippy runs are clean.
- **Verification**: 21 routes at 1280px and 320px, light and dark: 200 (404 for a missing id), axe 0 violations, no horizontal scroll, one `h1`. All four edit forms and New task refused (empty name: `Error:` title, one invalid field) and saved unchanged, a task created and refused as a duplicate, drag and drop on the agenda and the matrix (and back), keyboard pick on both boards, all on a copy of the database.


## Batch C (wording, tokens, docs; visible text and CSS values changed on purpose)

- **S1 empty states**: "No X yet." for lists that fill up: "No tasks yet.", "No sessions yet.", `NO_PROJECTS` = "No projects yet." on Home, Boards, Org info, Board info and both project checklists; Home's box "No projects yet. Create one with `iter init` or `iter new <path>`."; the board hint "No unfinished tasks yet. Add some to the board's projects, and they show up here."; the agenda lists "No important tasks waiting." / "No other tasks waiting."; the matrix zones keep "Nothing to do first/plan/delegate/eliminate." and "Everything is placed."; "No sessions in this period." stays (a real filter).
- **S2**: the task panel and the edit form both say "GitHub issue", "Start", "Duration" (the Schedule group carries "planned").
- **S3**: `FormError::new` runs the message through `sentence` (capital, period, " github " to " GitHub "), so the CLI's `IterError` strings are untouched. The task form's own refusals: "GitHub issue 'x' is not valid. Use a whole number such as 42.", "Start time 'x' is not valid. Use 2026-09-29 14:30.", "Duration 'x' is not valid. Use hours:minutes, such as 01:30."; the drop endpoint says "Start time 'x' is not valid." Hint tails: "Leave empty for none." (start, tags, issue, branch template, GitHub project); the branch prefix keeps "Leave empty to follow the project's template." (different meaning).
- **S5**: the Boards eyebrow stays "Board" (the section name, as the top bar). "Plan your day" was dropped from the docs. Making it "Boards" would only repeat the h1.
- **S6, S9, D7**: see `doc/web_final_review.md`. Not done: the `--t-xs/sm/md/lg` type scale (about a dozen sizes, so four tokens would change values), spacing tokens (deliberate).
- **Left on purpose**: the `.v2` scope prefix (a wide edit, no user gain); pagination (see batch A); `tasks_of` sorting in Rust and the tag rewrite on save.
