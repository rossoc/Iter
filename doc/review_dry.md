# DRY review: web UI (feat/html-refactor)

## Scope

- `src/web/**` (including `forms/`, `ui/`, `*.css`, `board.js`, `org.js`).
- `src/commands/**` only where it overlaps a web handler: task, project, board, sync.
- Also read `models/task.rs`, `models/project.rs` and `db.rs` where a duplicated rule lives there.
- Read-only. No `src` edits. The doc/web_*.md files were not read for findings.

## Method

What I did:

1. **Clone detector.** A Python script in the scratchpad (not in the repo): `scratchpad/clones.py`, and `clones2.py` (a variant that also collapses identifiers). It does these steps:
   - Strip comments.
   - Replace string and number literals with placeholders.
   - Slide a 4-5 line window over every `.rs` and `.js` file under `src/web` and `src/commands`, and index the windows.
   - Report windows that occur more than once, merged by proximity.
2. **CSS check.** A second small script lists CSS declarations longer than 38 characters that appear in more than one file. Greps counted hex colours, `px`/`rem` values, `@media` breakpoints, `z-index` and magic constants.
3. **String and magic-value greps.** I grepped for repeated string literals, query-parameter names, asset paths, route names and numeric constants in `.rs` files. I cross-checked the web side against `commands/` and `models/`.
4. **Manual triage.** I read the flagged files in full: `forms/*`, `edit*.rs`, `task_new.rs`, `project_new.rs`, `url.rs`, `quadrants.rs`, `matrix.rs`, `agenda*.rs`, `drop.rs`, `settings.rs`, `board.js`, `org.js`, `control.css`, `v2.css`, and the relevant parts of `commands/task.rs`, `commands/project.rs`, `commands/board.rs` and `db.rs`.

What the tooling found: textual clones are few. The exact-clone pass returned about 25 clusters, nearly all structural (`use topcoat::{...}` blocks, `#[component]` signatures, test fixtures). The real redundancy is semantic: the same fact (a constant, a wording, a rule, a size) written in two or more places with nothing linking them. The findings below come mostly from the string, constant and cross-file passes, not from the clone clusters.

What I did NOT check:

- I did not run `cargo build`, `cargo test` or `cargo clippy`.
- I did not run the server or look at any rendered page.
- Every finding is **inferred from reading and grep** unless marked "verified by running". The only things I verified by running are the script outputs (cluster lists, grep counts, CSS declaration counts) quoted below.
- I did not check `src/` outside the scope above. That includes `utils/`, `reporting.rs`, `git.rs` and `github.rs`, except where I cite them.
- I did not measure whether CSS rendering matches the coupled numbers in F4 and F7. They are coupled by value, not by a reference I could test.

## Findings

Severity: **High** means a likely divergence bug. **Medium** means a plausible bug on the next edit. **Low** means incidental or cosmetic.

### F1 (Medium): The CLI and the web write a task and its tags by different paths

- `commands/task.rs:89-93` (new) does `db.insert(&edit.task)?; db.set_task_tags(id, &tag_ids)?`.
- `commands/task.rs:115-117` (edit) does `db.update(id, &edit.task)?; db.set_task_tags(...)`.
- Each statement is its own transaction, so a failure in the second leaves the first applied.
- `web/forms/task.rs:134-141` goes through `Db::save_task` (`db.rs:581`), which wraps both in one `atomically`.
- `save_task` is `#[cfg(feature = "web")]`, so the CLI cannot use it.
- Validation is also split. The CLI does `require_name` plus `ids_by_name` in `checked_tags` (`commands/task.rs:101-104`). The web does the same in `TaskForm::apply` and `tag_ids`.
- Fix:
  - Drop the `cfg(feature = "web")` gate on `save_task`.
  - Call it from `commands/task.rs` in both places.
  - Move `checked_tags` into the model or `db` layer, next to `ids_by_name`.

### F2 (Medium): The "new task" and "new project" templates are rebuilt in web and CLI

- `web/task_new.rs:76-82` builds `Task::template(project.id(), git::branch_prefix(&project.branch_template), config().task.status)`.
- `commands/task.rs:67-72` is the same expression. `commands/sync.rs:81` and `:98` build it with their own status and the same `branch_prefix` call.
- `web/project_new.rs:132-143` (`Home::template`) does `Project::template(&config().project)` then `inherit_from(&org)`.
- `commands/project.rs:61-67` (`new_project_template`) is the same, with the organization resolved by name instead of by id.
- If a default changes (for example the status config, or a new field inherited from the organization), a caller can be missed.
- Fix:
  - Add `Task::for_project(&Project, status)`.
  - Add `Project::template_in(&Config, Option<&Organization>)`.
  - Make the three web and CLI sites call them.

### F3 (Medium): Field-to-error mapping depends on message prefixes

- `forms/task.rs:63-65` defines the prefix constants `BAD_ISSUE`, `BAD_START` and `BAD_DURATION`.
- `forms/task.rs:74-76` routes an error to its field by `m.starts_with(BAD_*)` on an `IterError::CommandFailed(String)`.
- The same constants format the messages at `:95`, `:109` and `:119`.
- The coupling is a string convention. If anyone rewords a message, or another producer emits "Duration ...", the error silently loses its field marker. It would still render, just unattached to the field.
- `settings.rs:55-90` has the same pattern, with an `about: &'static str` tuple carried beside the error.
- Fix: add a structured variant, for example `IterError::InvalidField { field, message }`. Then `field()` is a plain match with no string tests. That also removes the `CommandFailed` abuse.

### F4 (Medium): The date and time format and its examples are spelled in many places

- The format is `START_TIME_FMT` (`models/task.rs`).
- The literal example `2026-09-29 14:30` appears in `forms/task.rs:109` (error text) and `task_fields.rs:34` (hint).
- The wording "hours:minutes ... 01:30" is in `forms/task.rs:119` and `task_fields.rs:36`.
- The wording "yyyy-mm-dd hh:mm" is in `models/task.rs` (serde errors), `agenda_drop.rs:25` and `agenda_day.rs:93`.
- Another message, "Start time '{target}' is not valid." (`agenda_drop.rs:34`), has no usage example and a different tail from the form's version.
- The hint text and the error text for the same field are maintained separately.
- Fix:
  - Put the hint and the error wording per field next to `START_TIME_FMT` and `Duration`, as consts or `fn`s.
  - For example `const START_HINT`, `const DURATION_HINT`, `fn bad_start(raw)`.
  - Use them from `task_fields.rs`, `forms/task.rs` and `agenda_drop.rs`.
  - Derive the example from a fixed `NaiveDateTime` rendered with `START_TIME_FMT`, so it cannot disagree with the parser.

### F5 (Medium): Query-parameter names, values and anchors are re-spelled around `url.rs`

`url.rs` says it is the one place URLs are built, but the same names and values exist outside it:

| Item | In `url.rs` / `pick.rs` | Elsewhere |
|---|---|---|
| `side=off` | `url.rs:94` (writer) | `ui/day_nav.rs:56` (hidden input); readers `agenda.rs:71` and `matrix.rs:89` (`query.side.as_deref() == Some("off")`) |
| `pick`, `moved` | `pick.rs:72`, `pick.rs:86` | `ui/day_nav.rs:53`; `AgendaQuery` (`agenda.rs:43-52`) and `MatrixQuery` (`matrix.rs:50-58`) declare the same three fields and doc comments |
| `new=task`, `new=project` | `task_new.rs:70`, `project_new.rs:107` | `folders.rs:170` (hidden input); readers `org.rs:121`, `org.rs:125`, `project.rs:87`, `board_info.rs:37` |
| `tab`, `q`, `from`, `to`, `date` | `url.rs`, `org_report.rs:100` | `ui/filter_bar.rs:47`, `org_report.rs:164-166`, `agenda.rs:242`, `agenda.rs:266`, `ui/day_nav.rs:51` |

- A typo in a reader or hidden input, or a renamed writer, would compile and break only at runtime.
- Fix:
  - Put `const SIDE: &str = "side"`, `const FOLDED: &str = "off"`, `const NEW: &str = "new"` and the other names in `url.rs`.
  - Export an `is_folded(Option<&str>) -> bool`, and a shared `BoardQuery { pick, moved, side }` that Agenda and Matrix embed or both use.
  - Make the hidden inputs and readers use the constants.

### F6 (Medium): Magic target strings

- `quadrants.rs` has four quadrant targets (`"both"`, `"important"`, `"urgent"`, `"neither"`, lines ~35-64), and the table is the single source for them.
- `"left"`, the fifth target, is a bare literal at `quadrants.rs:110` (consumer) and at `matrix.rs:166`, `:190` and `:194` (producers).
- Fix: `pub const BACKLOG_TARGET: &str = "left"` next to `QUADRANTS`, and use it in all four places.

### F7 (Medium): The same hard-coded sizes appear in CSS, JS and Rust

- **Calendar icon width.** `org.js:~32` hard-codes `edge - 14 - 8`, the 14px width and 8px left margin of `::-webkit-calendar-picker-indicator`. The CSS source is `ui/control.css:21` (`width:14px;height:14px;margin:0 0 0 8px`). Change one and the hover target in the other silently misses.
  - Fix: expose `--cal-w` and `--cal-gap` as custom properties and read them with `getComputedStyle` in JS.
- **Sticky heights.** `v2.css:74` hard-codes `--sticky-head-h:76px` and `--pickbar-h:164px`, which are measured heights of the `.sticky-head` and `.pickbar` blocks. Anchor offsets (`scroll-padding-top`) depend on them. A font or padding change in `page_header.css` or `pick_bar.css` makes the anchors land wrong.
  - Fix: let the element publish its height via `ResizeObserver`, or document and test the equality.
- **Hours per day.** `[Vec<Card>; 24]` at `agenda_day.rs:19` and `(0..24)` at `agenda.rs:177` (also `now_line` assumes the hour index). Fix: `const HOURS: usize = 24`.
- **Default duration.** `Duration(60)` at `agenda_drop.rs:36`, and the same assumption in the doc comment at `agenda_drop.rs:25-26` ("an hour"). Fix: `const DEFAULT_DURATION: Duration` in the model.

### F8 (Medium): Responsive breakpoints are repeated across 15 CSS files

- `@media (max-width:900px)` appears 9 times (verified by running a grep), and `(max-width:600px)` 5 times. The paired `(max-height:600px)` appears in `v2.css:78`, `sticky_head.css:4`, `side_rail.css:22`, `side_lists.css:12` and `panel.css:11`. `jump_link.css:5` uses `min-width:901px` as the inverse.
- `v2.css:11` documents the "900px" breakpoint in a comment, but CSS custom properties cannot be used in media queries, so each file copies the number.
- Changing one without the others splits the layout. Today the sticky offsets in `v2.css:78` and the layout switch in `columns.css:11` must flip at the same width.
- Fix, either of:
  - Use a CSS build step (all CSS is already concatenated in `ui/mod.rs:11-60`) that substitutes `@media (max-width:var(--bp-md))`. This is cheap with a `str::replace` in `Asset::css`.
  - Keep the numbers but add a comment table in `v2.css` and a test that greps all CSS for any other `max-width` value.

### F9 (Low-Medium): Control and touch-target sizes are approximately-equal magic values

- Heights of 36px, 38px and 44px are all used for "a control": `org.css:5` (`--control-h:36px`, scoped to `.period` only, with a `var(--control-h,36px)` fallback at `org.css:30`), `modal.css:11` (36px), `hour_grid.css:7` (36px), `filter_bar.css:5-6` and `day_nav.css:19` (38px), `columns.css:8` (`--rail-w:44px`, equal to `--touch`).
- `--control-h` is defined only inside `.period` and `.copy`, so the fallback is the real value elsewhere.
- Fix: define `--control-h` once in `v2.css` beside `--touch`, and replace the loose 36px and 38px values with it (or with `--control-h-sm`).

### F10 (Low-Medium): The same form labels, hints and field specs appear in several screens

- **Name and Description.** `edit_page.rs:96-97` and the modals at `task_new.rs` (the `field(name: NAME ...)` and `textarea(name: DESCRIPTION ...)` lines of `task_modal`) and `project_new.rs:~182-183` (`project_modal`) repeat `field(name: NAME, label: "Name", required: true, identifier: true, error: error ...)` and the Description textarea. Only `autofocus` differs.
  - Fix: a `name_and_description(name, description, error, autofocus)` component.
- **Base path.** Both `project_edit.rs:83` and `project_new.rs:184` carry the base-path hint ("The project's directory on disk. ~ is expanded and a relative path is made absolute..."), the label, `required: true` and `identifier: true`. The new-project wording differs and is longer.
  - Fix: a shared `base_path_field` with a `beside` option.
- **Fixed filler strings.** The text "No organization" appears 4 times, "Use this folder" 4 times, and "Organization" 3 times (grep).

### F11 (Low-Medium): Parallel `Home` / `TaskHome` enums, and parallel "screen" pairs

- `project_new.rs:56-130` (`Home::{Org, Board}`) and `task_new.rs:50-75` (`TaskHome::{Project, Org}`) have the same shape. Each has `base`/`page_url`, `open_url` and a `match` that picks the loader (`org_screen` vs `board_screen` / `project_screen`).
- `task_new.rs::save` and `project_new.rs::list_page` re-implement the same "reload the home page with a modal open" `match`.
- `org_edit.rs`, `project_edit.rs`, `task_edit.rs` and `board_edit.rs` repeat the `show` / `save` / `screen` skeleton. This is acceptable because `EditForm` and `edit_page` already carry the shared logic. The remaining duplicate code is the `use topcoat::{...}` import block (4 times, per the clone report), page registration and the `path_param::<Id>` line.
- Fix: collapse the `TaskHome` / `Home` pair into one `ModalHome { kind, id }` with a `fn reload_page`. Replacing the skeletons with a macro (`edit_routes!`) is a judgement call: the current explicit shape is easy to read.

### F12 (Low): Labels and vocabulary

- "Backlog" is the label for the matrix's unplaced list (`matrix.rs:102`, `:190`, `:192-194`, five occurrences, plus tests `matrix.rs:230` and `:247`). On the agenda, "Backlog" is a sub-list (`agenda_cards.rs:104`) and "Unscheduled" is the column (`agenda_cards.rs:26`, `agenda.rs:104`). The CSS and docs call the same slot "side".
- One word means two things, and each label is a bare literal. Fix: `const BACKLOG: &str` in `matrix.rs`; leave the agenda names (they describe a different grouping).
- Date text. `"%a %-d %b"` is written at `agenda.rs:154` and `agenda_cards.rs:58`; `"%a %-d %b, %H:%M"` at `agenda_pick.rs:30`. Fix: constants next to `CLOCK_FMT`.
- `agenda_cards.rs:89-106` repeats `unplaced_list(...){ for card ... task_card(...) }` three times. It could be a loop over a `[(id, title, empty, &Vec<Card>)]` array. Low value.

### F13 (Low): Asset paths are spelled three times each

- `/v2.css` appears in `ui/mod.rs:121` (route) and `ui/frame.rs:54` (link).
- `/org.css` appears in `org.rs:47` (route) and `org.rs:189` (`styles`).
- `/org.js` appears in `org.rs:52` and `org_report.rs:173`.
- `/board.js` appears in `layout.rs:34` and `ui/board_root.rs:33`.
- `assets.rs:1-2` lists them in a doc comment.
- Fix: `pub const V2_CSS: &str = "/v2.css"` etc. in `assets.rs`, used in both the route attribute (not possible with the macro, so use a test that asserts every route path equals its constant) and the view.

### F14 (Low): Test fixtures are duplicated

- `Project::template(&Settings::default())` followed by `p.name = ...; p.base_path = "/tmp/..."` appears 22 times (grep), 10 of them in `web/` (`forms/task.rs:217`, `forms/mod.rs:193`, `forms/project.rs:71`, `task_query.rs:327`, `load.rs:120/152/183`, `footer.rs:79/84`, `project_new.rs:372/398`).
- A `Board { id: None, name, description: String::new() }` literal is built 7 times. `Db::open(":memory:")` appears 11 times in `web/` and `commands/`.
- Per-file helpers `task()`, `card()`, `day()` and `at()` exist in 13 test modules, and the dates `2026-09-28 09:00`, `2026-09-29 09:00` and `2026-09-30 09:00` recur (about 17 times). Fix: a `#[cfg(test)] mod fixtures` with `project(db, name)`, `board(db, name)`, `task(...)` and the sample day.

### F15 (Low): CSS value repetition (incidental)

- `transition: color var(--fast) var(--ease-out)` appears in 8 files, `background ...` in 3, and the combined form in 3 (script output). Fix: `--t-color`, `--t-bg` tokens, or a utility class.
- `z-index` is a bare scale (1, 5, 10, 20, 20, 30) across `frame.css`, `sticky_head.css`, `filter_bar.css`, `side_rail.css` and `modal.css`. Fix: `--z-*` tokens in `v2.css`.
- `#24283b` appears twice in the dark block (`--bg` and `--on-accent`, `v2.css:37` and `:47`). Fix: `--on-accent: var(--bg)`.
- Accent/ok mix ratios (`color-mix(... 45%, transparent)`) repeat in `status.css`. Fix: a token.
- `font-size:.8125rem` / `.875rem` appear 38 times in total (20 and 18). Fix: a type-scale token pair (`--fs-sm`, `--fs-xs`).

### F16 (Low): Table markup

- `ui/task_table.rs:50-56` and `ui/sessions_table.rs:35-99` repeat `role="row"`, `role="cell"` and `<th scope="col" role="columnheader">` (the string `columnheader` appears 9 times). The two files' header and row boilerplate is parallel.
- Fix: a `th(text)` and `td` helper, or drop the redundant roles on native table elements.

## Confidence

- **High** on the existence of each duplicate: every F-number rests on a file:line read, and the counts come from grep or the scripts that ran.
- **High** that F1 (the non-atomic CLI write) is real code behaviour, from reading `commands/task.rs` and `db.rs:581`. I did not run a failure injection to see it lose data.
- **Medium** that F3, F5, F6 and F7 would break at runtime on a change. I did not try the changes.
- **Medium-Low** on the severity of F8, F9, F12-F16. They cost maintenance, not correctness today.
- I found no case of two copies that already disagree in a user-visible way. The closest are the base-path hint (`project_edit.rs:83` vs `project_new.rs:184`) and the start-time error (`forms/task.rs:109` vs `agenda_drop.rs:34`), which differ only in wording.

## What I could not assess

- Whether the CSS coupled values in F7 are currently right. The sticky heights (76px and 164px) and the icon geometry need a browser.
- Whether `cfg(feature = "web")` gating is deliberate to keep the CLI binary small. F1's fix depends on that.
- Duplication in `utils/`, `git.rs`, `github.rs` and `reporting.rs` that mirrors web code. I only looked where web code called into it.
- Whether `role="row"` / `role="cell"` (F16) are needed for the `display:grid` table styling. I did not read the CSS rules that depend on them.
- `org.css` and `v2.css` token fallbacks that may be intentional compatibility shims.
