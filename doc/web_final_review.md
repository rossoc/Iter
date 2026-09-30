# Final whole-codebase review of `src/web/`

Five reviewers audited the whole of `src/web/` after the cleanup that removed `?design=old`. Each read the code only: none ran the server or a browser. This file records their findings, condensed from their reports, with the status after the last fix agent (which stopped early).

The per-page reviews (five lenses per page, pages 3 to 12) are not repeated here. They are in the "Review" table at the end of each `doc/web_<page>.md`.

Status key (batch C statuses are in the rows below): **done** = applied, builds, tests pass (271 with the web feature). **verified** = also checked in a browser or against the CLI (batch A). **open** = not applied yet.

---

## 1. Accessibility

Overall: skip link, `<main id=main tabindex=-1>`, single `<h1>`, titles with the "Error: " prefix, `lang="en"`, labels, fieldsets, `aria-describedby`, `aria-invalid`, table captions and mobile roles, pick mode, reduced motion and 320px reflow all looked sound. No regressions from the cleanup.

| # | Severity | Finding | Fix | Status |
|---|---|---|---|---|
| A1 | medium | Two `aria-current="page"` on one page. Org report: the period presets (`org.rs:246-248`) and the Report tab (`tabs.rs:104`). Board pages: the main-nav "Board" link (`frame.rs:55`) points at `/boards`, not the page shown, and sits next to the current tab or the "Edit" crumb | `aria-current="true"` for presets and the nav marker; keep `page` only for tab and crumb | verified (report: Report=page, preset=true) |
| A2 | medium | Forced-colors loses the current-item cue on the main nav (`frame.css:8`) and the period presets (`org.css:14-16`) | `@media (forced-colors:active)` underline, border or outline | verified (forced-colors: Report underlined, preset outlined) |
| A3 | low-medium | Org report error message not announced: `notice(...)` at `org.rs:301` has no `role="alert"` | `notice(error: true, ...)` | verified (`role=alert` on an invalid date) |
| A3b | low | Report date fields auto-submit (`org.rs:253-258`); the only submit button is `.sr` with `tabindex=-1` | Keep a visible or tabbable Apply, auto-apply stays as an enhancement | verified (Tab from To reaches Apply, `type=submit`) |
| A4 | low | Error-summary link to a checklist (`notice.rs:63`) targets a `<fieldset>` id, which is not focusable | Link to the first checkbox or give the fieldset `tabindex=-1` | verified (`#f-project` focuses the fieldset on both edit pages; no current error links to it) |
| A5 | low | Secondary `.button` border uses `--line-2`, about 1.4:1 in light mode (`button.css:4`), below 3:1 | Use `--control-line` | verified (`#857f76` on `#f4f2ee`) |
| A6 | low | `.empty-line` (`--faint`) drops to about 4.0:1 inside `.drop.over` during a drag | `.drop.over .empty-line{color:var(--muted)}` | done |
| A6b | unverified | topcoat's 400/404 pages are not built through `frame`, so they may lack skip link, landmarks and title | Render error pages through `frame` | verified (404 and 400 on GET and HEAD: title, skip link, one main, one h1, home link, `/v2.css`; POST keeps raw 400/404; an unmatched URL such as `/nope` keeps topcoat's bare 404) |
| A7 | info | `side_lists` (`<aside tabindex=-1>`) is a scrollable container keyboard users cannot focus; the folded-list title is a `span` in `<summary>`, not a heading | Acceptable in practice | accepted |

Confirmed fine: focus outlines, scroll-padding under the sticky bar, touch targets, `.sr` captions, the `?moved` status region, `board.js` failure alert, pick-mode alternative to drag, reduced-motion guards.

---

## 2. Algorithmic complexity and performance

Verdict: no N+1 inside `src/web`. Every route runs a fixed number of queries, and the group lookups are batched. Two real problems: the shared `project_reports` N+1, and blocking SQLite inside async handlers.

Queries per GET route (from the code):

| Route | Queries |
|---|---|
| `/` | 2 |
| `/boards` | 2 |
| `/board/{id}`, `/board/{id}/matrix` | 5 |
| `/board/{id}/info` | 2 |
| `/board/{id}/edit` | 3 |
| `/org/{id}?tab=info` | 3 |
| `/org/{id}?tab=tasks` | 3 |
| `/org/{id}?tab=report` | 2P+3 (P projects) |
| `/org/{id}/edit` | 3 |
| `/project/{id}` | 3 |
| `/task/{id}` | 5 |
| task edit, new task, project edit | at most 4 |

Indexes: every filter used in `src/web` is covered (`sessions(task_id,start)`, `projects(organization_id)`, `projects(board_id)`, `task_tags`, `tasks(project_id,name)`).

| # | Severity | Finding | Fix | Status |
|---|---|---|---|---|
| C1 | medium | `utils::report::project_reports` (`report.rs:~112-122`) runs 2 queries per project (sessions and tasks); shared with `iter org info` | One tasks query and one sessions query for the whole set via `json_each`; pure builder shared with the single-project CLI path; regression test old vs new | verified (63:36, 7 projects, 14 tasks, 41 sessions and the seven project totals equal `iter org info` for 2026-09-15..29; one day 01:02 equal; regression test green) |
| C2 | medium-high under concurrency | Blocking SQLite (`open_db`, WAL pragma, migrate, `busy_timeout` 5s) runs directly on tokio worker threads (`mod.rs:58`, `db.rs:219-233`). A held write lock can stall the whole server. Each request opens a fresh connection, so `prepare_cached` never hits | Wrap DB sections in `spawn_blocking`, or one cached connection per blocking thread | done, differently: one layer (`blocking.rs`) runs each request in `block_in_place`. With the write lock held and 40 blocked writers, `/v2.css` took 4.3 s before and 5 ms after |
| C3 | medium at scale, low today | Unbounded payloads: org report "All time"; `sessions_for_task`; Tasks tabs; board pages render every unfinished card. Report scripts (`REPORT_JS`, `COPY_JS`, ~2 KB) are inlined on every response | Cap or paginate; serve the scripts once from static routes | scripts done (`/org.js`); pagination not needed now (see `web_cleanup.md`) | |
| C4 | low | `/v2.css` is about 37 KB from a `concat!` constant (no per-request work), but `Cache-Control` / ETag could not be confirmed | Check with curl; add a caching header or a hashed URL | done: there was none; `assets.rs` adds ETag and `Cache-Control: no-cache`, 304 verified |
| C5 | low | `tasks_of` clones the project name per task and sorts by string (`db.rs:~890-915`) | `ORDER BY p.name, t.name` in SQL | skipped: not trivial (see `web_cleanup.md`) |
| C6 | low | POST drop: 2 reads outside the transaction, then a full-row UPDATE (lost-update race, not a speed issue); `save_task` rewrites tags even when unchanged | Optional join check; skip-if-equal on tags | skipped: not trivial (see `web_cleanup.md`) |
| C7 | low | `merged_total_minutes` clones and sorts spans at three levels | Negligible; optional | accepted |

---

## 3. Composability

| # | Severity | Finding | Fix | Status |
|---|---|---|---|---|
| P1 | medium | `ui/` still holds domain and URL logic. `ui/url.rs` (151 lines of `*_url` builders) is re-exported through `ui/mod.rs:70`; `ui/tabs.rs` does four jobs (`Tab`+`tabs`, `Section::of` parsing `?tab=`, `Section::title`, `section_tabs`); `ui/prose.rs:20` `first_line` is text mapping; `ui/project_list.rs:10` `tilde()` reads `$HOME`. Imports of `models` and `error` in `ui/` (`TaskStatus`, `Priority`, `is_hex_color`, `IterError`) are vocabulary types | Move `url.rs` to `web/url.rs`; `Section`/`section_tabs`/`tabs_for` to `web/sections.rs`; `first_line` to a web mapping module; `tilde` into `project_items`. Keep the domain imports and correct the docs ("ui depends on nothing above it") | verified (batch B): `url.rs` and `sections.rs` and `notes.rs` are in `web/`, `tilde` is in `project_items`; `ui` imports only vocabulary types; every route byte-identical, axe 0 |
| P2 | medium | Page components are still named `proposal` (11 files), and `pages.rs:13-20` re-aliases them (`org_edit_form`…). `web_cleanup.md` says they are `screen` | Rename to `screen`, drop the aliases | verified (batch B): all page components are `screen`; `pages.rs` aliases gone |
| P3 | medium | Route ownership is inconsistent. `pages.rs` (221 lines) holds routes and loaders for eight pages while the board pages own their routes. `agenda.rs` (421 lines) mixes routes, drop logic, `Cards`, `side`, `now_line`; `org.rs` (389 lines) mixes Info, the whole Report feature and the `/org.css` route; `forms.rs` (721 lines) holds four forms plus helpers | Each page owns route + loader + screen and `pages.rs` goes away; extract `org_report.rs`; split `agenda.rs`; split `forms.rs` into `forms/{mod,org,board,project,task}.rs` | verified (batch B): `pages.rs` deleted, each page owns route + loader + `screen` (`show`/`save`), `layout::register`, `org_report.rs`, `agenda_drop.rs`, `agenda_cards.rs` (`Cards` and `side`, not `board_cards.rs`: they are agenda-only and use the agenda's pick links), agenda `Model` outside the component, `forms/{mod,org,board,project,task}.rs` |
| P4 | low-medium | `board_edit.rs:62-74` bypasses `edit::load` (inline `list::<Board>()` + `swap_remove`, and a second read at line 87); the Projects checklist with hint, `empty` and `PROJECTS` name is duplicated in `org_edit.rs:38-46` and `board_edit.rs:46-54`; `org_in` in `project_edit.rs:14` is a lookup helper in a page file; `task_new.rs` has six free functions for one form (`template`, `blank`, `load`, `start`, `refused_parents`, `proposal`) that overlap; each edit page recomputes `base`/`action`/`cancel` | Use `edit::load` in board edit; one `project_checklist(options, noun, error)`; move `org_in` to `load.rs`; fold `task_new` into one small API | partly (batch B): `project_checklist` shared, `org_in` in `load.rs`, `NewTask` folds the `task_new` functions, board edit reads through `edit::load_among` (a sibling of `load`: `edit::load` itself would add a read by key to a page documented as two queries). Not done: the refused-POST list read (needed for the "currently in" names), an `EditForm` hook for base/action/cancel (each page still writes three lines; a hook would add a trait method per form) |
| P5 | low | `ui/tasks_tab.rs:22-32` writes the add-button paragraph twice; multi-component files (`panel.rs` five components, `field.rs` nine functions); pick URLs live in two modules (`pick.rs`, `agenda_pick.rs`); naming clashes (`quadrants.rs`/`ui/quadrant.rs`, `task_rows.rs`/`project_rows.rs`) | Render once; split only if mechanical; unify pick URLs | partly (batch B): the add button is one component, `PickLinks` replaces the two pick-URL modules' helpers. Not done: `quadrants.rs`/`ui/quadrant.rs` rename, splitting `ui/panel.rs` and `ui/field.rs` (each has one sheet; splitting is not mechanical) |
| P6 | info | Sound: `EditForm` + `edit_page` skeleton, `path_param!(id)` centralised, `crumbs.rs`, `load.rs`, `project_rows.rs` and the other mapper modules are well placed. Single-use `ui` components (`day_nav`, `hour_grid`, `matrix_grid`, `quadrant`) are self-contained and fine | — | accepted |

---

## 4. DRY

Overall: already well factored. Routes are built in one place, the four edit pages share `edit_page.rs`, both boards share `board_page.rs`, `pick.rs` and `drop.rs`. No CLI logic is duplicated beyond the report N+1 above.

| # | Worth | Finding | Fix | Status |
|---|---|---|---|---|
| D1 | worthwhile | Tasks-tab load written twice: `pages.rs:68-78` and `project.rs:44-52` | One `tasks_or_count` helper in `load.rs` | verified (batch B): `load::tasks_or_count` |
| D2 | worthwhile | Agenda and matrix duplicate shells: the `lead` title string (`agenda.rs:120-124`, `matrix.rs:80-84`), the title-skip logic, and the pick verbs (`Picked::verb/action` in `agenda_pick.rs:14-25` vs `pick_words` in `matrix.rs:~98-103`) | `title_parts` helper; one `PickWords` per board | verified (batch B): `board_header::title_parts`, `pick::PickWords` (`of`, `title`, `MOVING`) |
| D3 | small | The project checklist loaders are written twice (`org_projects` in `pages.rs:~99-105`, inline in `board_edit.rs:71-91`) | `project_options` generic over the group | verified (batch B): `project_options::group_options` and `project_checklist` |
| D4 | trivial | Date formats repeated: `ui/url.rs:76` vs `reporting::fmt_date`; `session_lines.rs:32` hardcodes the `datetime` format; `%H:%M` repeated; `Local::now().naive_local()` repeated at three places | Use `fmt_date`; name the constant; `now()` helper | verified (batch B): `web::now()`, `url.rs` uses `fmt_date`, `DATETIME_ATTR`, `CLOCK_FMT` |
| D5 | trivial | Duration text has two formats: `duration_label` (`1h 30m`, cards) vs `Duration::Display` (`hh:mm`) | Document, or `Duration::short()` | documented on `duration_label` (batch B) |
| D6 | trivial | `proposal` naming mismatch with the doc | See P2 | verified (batch B): see P2 |
| D7 | small | CSS magic numbers: sticky heights (`--sticky-h:76px` in `v2.css:66`, `.toc{top:76px}` at `org.css:36`, `.proj{scroll-margin-top:72px}`, 164px pick bar); 44px touch target in four places; 1180px main width repeated in `frame.css:19` and `columns.css:4` (the info-body width silently depends on frame padding 2x72px); breakpoints repeated about 15 times (plain CSS cannot use variables there); `68ch`; `.8125rem` 18 times; body background hexes duplicated in `v2.css:44-45` because `--bg` exists only inside `.v2` | Tokens `--touch`, `--main-w`, `--sticky-head-h`, `--pickbar-h`; hoist `--bg` to `:root`; comment the breakpoints | verified (batch C): `--touch`, `--main-w` + `--main-pad-max` + `--main-pad` (frame and `--info-body`, computed 668px as before), `--measure`, `--sticky-head-h`, `--pickbar-h`, `.toc{top:var(--stick-top)}`, `.proj{scroll-margin-top:calc(var(--bar-h) + 20px)}`, `--bg` on `:root` (body uses it), breakpoints listed in one comment; 60 screenshots (15 pages, 1280 and 320, light and dark) compared before and after: only the intended empty-text and path colors differ. Not done: `.8125rem` and the other sizes (see S8) |

No findings: route strings outside `ui/url.rs` are only inside macro attributes and tests; `page_url`, `Section::of` and the `?tab=` parse are single-source; form validation is shared; `board.js` has no duplicated constants.

---

## 5. Design consistency

Overall: coherent. One header, tabs, button, notice, panel and table pattern each; motion is gated; colour carries meaning only (accent = current, wip, today, drop target; `--ok` = done; `--danger` = errors; priorities from tags).

| # | Severity | Finding | Fix | Status |
|---|---|---|---|---|
| S1 | medium | Empty-state wording follows four patterns: "No tasks." / "No sessions." / "No sessions in this period." / "No projects." / "No projects yet." / "No projects on this board." / "Nothing important waiting." / "Nothing to do first." / "Nothing here yet…" / "No boards yet…" / "This board has no unfinished tasks…" | "No X yet." for lists that fill up (projects, tasks, sessions); keep "Nothing to Y." only for the matrix zones; "in this period" only where it is a real filter | verified (batch C): "No tasks yet.", "No sessions yet.", `NO_PROJECTS` = "No projects yet." on Home, Boards, Org info, Board info and the checklists; Home box "No projects yet. Create one with `iter init` or `iter new <path>`."; board hint "No unfinished tasks yet. Add some to the board's projects, and they show up here."; agenda lists "No important tasks waiting." / "No other tasks waiting."; matrix zones and "No sessions in this period." unchanged |
| S2 | medium | Same field, different labels: task panel says "GitHub issue", "Planned start", "Planned duration" (`task_panel.rs:25-28`); the edit form says "Issue number", "Start", "Duration" inside a "Schedule" group (`task_fields.rs:33-42`) | One label per field: "GitHub issue", "Start", "Duration" (the Schedule group already carries "planned") | verified (batch C): panel and edit form use those labels |
| S3 | medium | Errors and hints do not read as one voice: hint tails differ ("Leave empty for none." / "for unscheduled." / "to follow the project's template."); refusals are lowercase fragments without a period (`forms.rs:326-328`, `605`); "github" lowercase in errors, "GitHub" in labels | Capitalised sentences with a period, "GitHub" everywhere, one hint tail. Map in the web layer, not the CLI messages, unless the CLI wording clearly improves | verified (batch C): `FormError::new` -> `sentence` (capital, period, " github " to " GitHub "); task form: "Start time 'x' is not valid. Use 2026-09-29 14:30.", "Duration 'x' is not valid. Use hours:minutes, such as 01:30.", "GitHub issue 'x' is not valid. Use a whole number such as 42."; hints end "Leave empty for none." (the branch prefix keeps its different meaning); the CLI strings are unchanged |
| S4 | low | Agenda side region "Not scheduled" vs its lists "Important, not scheduled" / "Scheduled on other days"; matrix says "Not placed" | Acceptable (schedule/place verb split); note it | accepted |
| S5 | low | Home eyebrow "Home" + h1 "Where to next?"; Boards eyebrow "Board" repeats h1 "Boards"; `doc/web.md` and `doc/web_boards.md:12` say "Plan your day" | Pick one; fix the code or both docs | done (batch C): the code keeps "Board" (the section name, as the top bar; "Boards" would repeat the h1), the docs say so |
| S6 | low | Two greys for "nothing here": `.empty-line` uses `--faint` (fails 4.5:1 on `--bg-2`, patched in three places), `.unset` uses `--muted` | `--muted` for empty text; `--faint` only for decoration | verified (batch C): empty lines and project paths use `--muted`, the three patches are gone; `--muted` is 5.1:1 on `--bg` and 4.7:1 on `--bg-2` in light, 5.2 and 5.5 in dark; axe 0 |
| S7 | low | Density of the busiest pages: agenda and matrix cards can show six things (edge, chips, status, time, title, action); the priority edge and the chips encode the same thing | Keep chips only where the place does not imply them (already true on the matrix) | accepted, watch |
| S8 | low | Literals outside tokens: no type scale (about 12 raw font sizes; `.8125rem` x17, `.875rem` x11), weights 400/500/550/600/650, letter-spacings -.015…-.035em; no `--s*` spacing tokens (deliberate); `breathe 2s` and `flash 1.6s` non-token durations; 76px/72px in `org.css:36,48`; widths 260/280/560/640/1180px; 14px chip radius (deliberate) | Optional `--t-xs/sm/md/lg` if the swap is mechanical; derive 76/72px from `--bar-h` | partly (batch C): 76/72px derived from `--bar-h`/`--stick-top`; not done: `--t-xs/sm/md/lg` (about a dozen sizes, four tokens would change values), weights and letter-spacings |
| S9 | low | `flash` animation is not `no-preference` gated (equivalent, disabled under `reduce`) | Gate it for symmetry | verified (batch C): `flash` is `none` under reduce, `flash` under no-preference |

### Design-wiki drift (fixed in batch C)

- **`design/04-direction-for-iter.md`:** still says "a proposal, not yet implemented" and points at the deleted `style.css`. Built differently: tokens are scoped on `.v2` not `:root`; dark theme is Tokyo Night (`#24283b`, accent `#ff9e64`) not near-black with a blue accent; the sidebar was removed; links use a sliding accent line; tabs slide in from the left; cards are `--bg-2` with a 4px edge; the now line is 2px with an 8px dot; focus rings are outlines; empty states are a 28px icon in a dashed box with no action link; the h1 is `clamp(2rem, 1.4rem+2vw, 3rem)`; `.label` is 11px at .09em; reduced motion zeroes `transition-duration` only, with animations gated by `no-preference`. Not built: the `--s1..--s7` scale, the `--t-*` scale, `--ease-std`, grain texture, `@view-transition`, `<kbd>`, `form:has(:invalid)`. Undocumented tokens that exist: `--control-line`, `--slow`, `--line-w`, `--head-gap`, `--edge-w`, `--q1/--q2`, `--bar-h`, `--sticky-h`, `--stick-top`, `--form-gap`, `--num-w`, `--quad-gap`, `--quad-min`, `--info-aside`, `--info-gap`, `--label-w`.
- **`design/README.md`:** "one hand-written stylesheet" is now one concatenated sheet at `/v2.css` (from `ui/*.css` plus `v2.css`) with `org.css` as a page sheet; add a "built" status note.
- **`design/03-scheduling-apps.md`:** "3px left border" should be 4px.
- **`doc/web.md`, `doc/web_boards.md`:** the "Plan your day" eyebrow (S5).
- **`doc/web_components.md`:** mentions a `home.css` that no longer exists, "11 components" (now about 37 sheets), and a token list missing `--line-w`, `--head-gap`, `--slow`; documents `task_table` with the old `new_in` signature.
- **`doc/web_cleanup.md`:** should note that the `.v2` scope is a leftover and that 76px/72px in `org.css` are not tied to `--bar-h`.

---

## Summary of what remains

Verified in batch A (browser at 1280px and 320px, light and dark, axe 0 violations; CLI comparison): A1-A6b, C1. Done in batch A: C2 (layer), C4 (ETag), the report scripts served from `/org.js`. Skipped with reasons: C5, C6 tag rewrite, pagination.

Done in batch B (structure and duplication, verified byte-for-byte and in a browser): P1-P3, D1-D6; P4 and P5 partly (see their rows).

Done in batch C: D7, S1-S3, S5, S6, S9, S8 in part, the design-wiki and doc updates, and the end-to-end pass (see `doc/web_report.md`). Still open: S8 type scale and weights, the follow-ups in `doc/web_cleanup.md`.
