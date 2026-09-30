# Task edit redesign (`/task/{id}/edit`)

> Superseded (final cleanup, `doc/web_cleanup.md`): the compare switch and `?design=old` were removed; the pages render the current design only. References below to the old design, the switch or `proposal` (now `screen`) describe how it was built, not how it is.


Scope: the task edit form (GET shows it, POST saves). It is the sibling of Project edit (`doc/web_project_edit.md`) and the twin of New task (`doc/web_project.md`), and builds on the task page (`doc/web_task.md`) and the shared components in `src/web/ui/` (`doc/web_components.md`). It adds no component: everything it needs already exists.

## Plan

### Page
1. Same frame as Project edit (`ui::frame`, no sidebar) through `edit_page` (the skeleton shared with Org edit, Project edit and New task). Task edit and New task differ only in the crumbs, the heading, the submit label and the form's target; the fields are the same component, `task_fields`, and the values are the same type, `TaskForm`.
2. Header: `page_header` with the breadcrumb as kicker, `Organization / Project / Task / Edit` (organization only when there is one; the project linked to its Tasks tab and the task linked to its page, which is where Cancel goes) and the title "Edit {name}". The crumbs are `crumbs::edit_trail` (`trail` down to the task, linked to its page, then "Edit"), the same one the Org and Project edit pages use: "an edit page sits under the page it edits". Document title "Edit task - Name - iter", prefixed "Error: " after a refused submit.
3. Form: Name (required) and Description (`edit_page`), then `task_fields` as on New task: Status, Tags, Schedule group (Start, Duration), GitHub group (GitHub issue, Branch prefix), with the same hints; Save / Cancel (back to the task page).
4. Values: `TaskForm::of(&task, &joined(tags))`. Tags are the task's tag names, comma-separated, read in one query (`tags_for_task`).
5. One file, `web/task_edit.rs`, like `project_edit.rs`: `values` (the form's first values, the twin of `task_new::blank`) and one component `proposal` that fills `edit_page` with its crumbs and `task_fields`. The GET's loading is `edit::load` with `parents_of` and `values` as its `extra`. No rules of its own, no CSS.

### Errors
6. As on the other edit pages: the message in the focused error notice linking to the field it is about, that field `aria-invalid` with the "Fix this" cue. `TaskForm::field` already names Name (empty, taken), Status, Tags (unknown tag), GitHub issue, Start, Duration.
7. A refused submit keeps everything typed (also text that does not parse, and the tags text), and the page's crumbs and title use the *stored* task (`Refused::stored`), so what was typed in Name never reaches the breadcrumb. A taken name reads "a task with this name already exists in this project".
8. Behavior does not change: same `TaskForm`, `apply`, `Db::save_task` (row and tags in one transaction, tag names resolved in one query before anything is written), 303 to `/task/{id}`.

### Comparing
9. `?design=old` on the GET shows the old form and sidebar with the switch (`compared`). The old form posts without the parameter, so a refused old submit comes back as the proposal. The old `task_form` no longer shows an error (a refused POST always renders the proposal), so its `error` parameter goes.

### Speed
10. GET: task row, then its project and organization by key (two lookups, the last only when there is one), and the task's tags (one query). No sidebar query (`Nav::load` only for `?design=old`). Refused POST: the task row once (`submit`), plus the same project, organization and tags lookups for the crumbs; the tags shown are the typed text, so no tag lookup is needed there.

### Old code
11. Deleted now: the `error` parameter and `error_box` call of the old `task_form`, and the old-shell rendering of a refused `task_save`.
12. Kept, each with its removal trigger:
    - `task_form` (old, GET only: `?design=old` of New task and Task edit), `field`, `textarea`, `check`, `form_actions` of `layout.rs` / `pages.rs`, `old_settings_fields`, `project_form`, the org form: the `?design=old` switches of the edit pages and the old Board edit page (page 12, the last old form); the final cleanup pass drops them with the switches.
    - `layout::error_box`: the old Board edit page (12) only; nothing else uses it once the old `task_form` loses its error.
    - `shell`, `Nav`, `Sel`, `cls`, `description`, `row`, `settings`, `task_table`: the old pages (`?design=old`) and the board pages.
    - `style.css` form rules: the old forms above, so they go with them.

### Feature changes
- Added: breadcrumb (organization / project / task / Edit); hints, groups, required marker and select as on New task; field-level errors; typed values kept after a refusal; a readable duplicate-name error; the compare switch (GET only).
- Changed: same look as Project edit and New task; the sidebar is gone from the page; "Start (yyyy-mm-dd hh:mm)" and similar labels become "Start" with a hint. Field order is by how often a field is changed (Status, Tags, Schedule, GitHub), the Details panel's by importance. The field is labelled "GitHub issue", as the panel row (batch C: one label per field; it was "Issue number"). Planned start is not a field: a task's start is what the Schedule group sets, and it is rarely known at creation.
- Removed: nothing.

### Loading
13. `edit::load` and `edit::submit` hand the stored row to their `extra` closure (`|db, row|`), so the task's project is found from its `project_id` in the same connection, on GET and on a refused POST alike, with no second read. The other callers ignore the row.

## What changed, checked against the plan

| # | Plan | Status | Where |
|---|---|---|---|
| 1 | `edit_page` and `task_fields` shared with New task | done; only crumbs, heading, submit label and target differ | `task_edit.rs`, `task_new.rs` |
| 2 | Crumbs, title, "Error: " title | done; "4BC / emoETL / required-args-crush / Edit", "Edit task - required-args-crush - iter" | `crumbs::edit_trail` |
| 3 | Fields, hints, groups | done | `task_fields.rs` |
| 4 | Tag text in one query | done; "Important, Urgent" after saving two tags | `task_edit::values` |
| 5 | One file, no new component | done | `task_edit.rs` |
| 6 | Field-level errors | done; empty name, issue, start, duration, unknown tag, duplicate name each mark and focus their field | `TaskForm::field` |
| 7 | Typed values and stored crumb/title | done; a typed name never reaches the crumb; duplicate reads "a task with this name already exists in this project" | `Refused::stored` |
| 8 | Same POST behavior | done; status, tags, start and duration saved (303 to `/task/3`) and restored | `forms.rs` |
| 9 | `?design=old` on GET | done; old form with sidebar and switch; its POST comes back to `/task/3` | `pages.rs` |
| 10 | Speed | done: no `Nav::load` by default; task, project, organization, tags | `pages.rs` |
| 11, 12 | Old code | error path of `task_form` and old-shell refusal deleted, `error_box` import gone from `pages.rs`; the rest stays with its trigger | `pages.rs` |
| 13 | `extra` gets the row | done | `edit.rs` |

Checked in a browser (throwaway database copy via `XDG_CONFIG_HOME`, server on port 3010 because 3000 was taken): GET of a task with tags and one without, successful save, refused cases (empty name, duplicate name, bad issue, bad start, bad duration, unknown tag; typed values kept), Cancel, `?design=old`. axe: 0 violations on GET (both tasks) and a refused submit, light and dark, 1280px and 320px, no horizontal scroll; the Task, Project, Org and New task pages also 0 and unchanged. Tab order: skip link, brand, Board, crumbs, Name, Description, Status, Tags, Start, Duration, Issue number, Branch prefix, Save, Cancel, the compare switch.

Verified: `cargo fmt --check`, `cargo clippy --features web`, `cargo test --features web` (217; new: task edit trail), `cargo test --no-default-features` (174).

Ordering of refusals: `apply` reports the first problem among name, issue, status, start and duration, in that order. Tags are resolved before the write (`TaskForm::save`), and the name is checked by the write itself, so an unknown tag is reported before a taken name.

## Review

Five reviewers (accessibility, complexity, composability, DRY, design). Checked in a browser again afterwards (port 3013, throwaway copy): Task edit, New task, Project edit and Org edit GET, save, refused cases (typed values kept, field marked, "Fix this" outside the label: the accessible name of Duration is "Duration"), Cancel, `?design=old`, the Task, Project and Org pages unchanged; axe 0 violations at 1280 and 320px, light and dark; Tab order unchanged.

| Finding | Status |
|---|---|
| A1 "Fix this" inside the label | fixed: `field::invalid_cue`, a visible `aria-hidden` sibling in `.field-head`; the error notice in `aria-describedby` says it to a screen reader |
| A2 date/time and duration hints | fixed: "year-month-day hours:minutes, for example 2026-09-29 14:30", "hours:minutes, for example 01:30"; no `inputmode` |
| A3 legend size and colour | fixed: .8125rem, `--muted` (above 4.5:1 in both themes); one step above the panel heading, on purpose |
| A4 first error only, role removed by the script | accepted, no change: the form reports one refusal at a time by design (`apply`), and the script removal of the role avoids a double read |
| C1 parents only when shown | fixed for the GET: `parents_of` runs only when not `?design=old`. Refused POST needs them, so it loads them. The `existing.clone()` in `submit` stays: `apply` consumes the row and the page needs the stored one (a few strings; a `&Row` API would clone inside `apply` instead). Refusal order documented above |
| P1 create counterpart | fixed: `edit::create` and `EditForm::insert`; `TaskForm::create` is gone; New task writes through the same `settle` as the edits; closures return `topcoat::Result` |
| P2 one `Parents`, one argument shape | fixed for the task pages: `load::Parents` and `parents_of` (404) used by `task`, `task_new`, `task_edit`, `project`; `task_edit::Parents`, `task::Loaded`'s pair and `NewTask` are gone; both task forms take `(stored, parents, values, error)`. Org and Project edit keep their own shape (typed row plus `stored`): unifying them means changing two finished pages for no gain until Board edit shows what it needs |
| P3 `values` / `load` symmetry | fixed: `task_edit::values`, `task_new::load`/`blank`; `tag_text` gone. The named per-page `extra` structs are skipped: the three tuples are two-element closures results in `pages.rs` |
| P4 one `edit_trail` | fixed: `edit_trail(&Option<Organization>, Option<&Project>, name, url)`, `task_edit_trail` gone, one test |
| P5 `ui::checklist` | fixed, rendered on a scratch route (removed) and checked with axe: 0 violations. `FormError::new` unchanged (churn) |
| D1 handler boilerplate in `pages.rs` | skipped: each POST handler is 6 lines and differs in what it passes; moving them into the modules moves the code without removing any |
| D2 doc fixes | fixed here, in `doc/web_project.md`, `doc/web.md`, `doc/web_components.md` |
| D3 old code | kept, trigger recorded in item 12: the last old form is Board edit (page 12) and the `?design=old` switches |

Verified: `cargo fmt --check`, `cargo clippy --features web`, `cargo test --features web` (219), `cargo test --no-default-features` (174).
