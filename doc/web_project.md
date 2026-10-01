# Project page redesign (`/project/{id}` and `/project/{id}/task/new`)

> Superseded (final cleanup, `doc/web_cleanup.md`): the compare switch and `?design=old` were removed; the pages render the current design only. References below to the old design, the switch or `proposal` (now `screen`) describe how it was built, not how it is.


Scope: the project page (Info and Tasks tabs) and the "New task" form (GET shows it, POST creates). The project edit form (page 5) is left for later and keeps working. It builds on Org (`doc/web_org.md`), the org edit form (`doc/web_org_edit.md`) and the shared components in `src/web/ui/` (`doc/web_components.md`). The page is the Org page's sibling: same header, tabs, two-column Info, task table.

## Plan

### Project page
1. Same frame as Org (`ui::frame`, title slot: "Project - Acme-app - iter", "Tasks - ..." on the Tasks tab). Header is `page_header` with the **breadcrumb as kicker** when the project has an organization (`Acme / Project`, the organization linked) and the "Project" eyebrow when it has none; `h1` is the name; the Edit button (`edit_button`) on the right.
2. Tabs (`ui::tabs`): **Info** and **Tasks**, the Tasks tab with a count. The count is one `COUNT(*)` query (new `Db::count_tasks_for_project`); the task rows are loaded only on the Tasks tab.
3. Info tab, in two columns like Org: left the description (line breaks kept) and **Base path** (mono, full path); right the **Settings** panel (`settings_panel`, shared with Org). The two-column layout and the description block, now written twice, become one component: `ui::info` (`info_columns`, `description`), taken out of `org.css`; Org uses it too.
4. Tasks tab: `task_table` without the Project column and with the **New task** link row at the top (the `add` slot, as the old table had). With no tasks the table holds only that row and `empty_line("No tasks.")` follows it.
5. Empty description: nothing shown, as on Org. Empty base path is not possible (it is required).
6. No sidebar, so no `Nav::load`: only the old design loads it. The organization is read only for the breadcrumb (one lookup, only when the project has one).

### New task (`/project/{id}/task/new`)
7. Same frame, no sidebar. Header: breadcrumb `Acme / Project name / New task` (organization when there is one, the project linked to its Tasks tab), title "New task". Document title "New task - Project name - iter", prefixed "Error: " after a refused submit.
8. The form is `ui::form` + `field` / `textarea` / `select` + `form_actions("Create task", Cancel back to the Tasks tab)`: Name (required), Description, Status (select), Tags (hint: comma-separated, Urgent and Important flag the task), then a Schedule group (Start, hint "year-month-day hours:minutes, for example 2026-09-29 14:30", empty for unscheduled; Duration, hint "hours:minutes, for example 01:30") and a GitHub group (Issue number, Branch prefix). The order is by how often a field is changed (a Planned start is rarely known at creation), the task panel's by importance. Fields are `identifier` where they are not prose. The eight inputs are one component, **`task_fields`** (`web/task_fields.rs`, one feature: the inputs of a task, read back by `TaskForm`), next to `settings_fields`: page 7 (task edit) will use the same one.
9. Errors as on the org form: the message in the focused error notice, linked to the field it is about, that field `aria-invalid` + `aria-describedby`. `TaskForm::field` names the field for: empty name (Name), invalid status (Status), bad issue (GitHub issue), bad start (Start), bad duration (Duration), unknown tag (Tags). The task edit page (page 7) gets the same for free.
10. Behavior is unchanged: the same `TaskForm::apply`, tags resolved before the task is inserted, 303 to `/task/{new id}`. A refused POST shows the form with what was typed.
11. `?design=old` on the GET keeps the old form and sidebar (with the switch). The old form's POST comes back as the proposal, as on the org form. The old `task_form` stays: task edit (page 7) still uses it.

### Speed
12. Project GET: one row, one org row at most, one `COUNT` (Info) or one task query (Tasks). New task GET: project row and org row. POST: as before plus the org row only when refused.

### Old code
13. `?design=old` of the project page and the new-task form still use the old `tabs`, `task_table`, `row`, `settings`, `org_crumb` and `task_form` of `pages.rs` (Org's old view and the task pages 6-7 use them too), so none is deletable yet; they go with the other `?design=old` branches at the end. What is deleted now: the old project route body (sidebar loaded on every request) and the old task-create POST duplicate (`new_task_of` serves GET and POST).

### Feature changes
- Added: breadcrumb with the organization on the project and new-task pages; task count on the Tasks tab; base path labelled; hints on Start, Duration, Tags; field-level errors on the task form; the compare switch on both GETs.
- Changed: same look as Org (frame, tabs, table, Settings panel); "+ New task" is an icon link row; the task form uses the shared controls; the sidebar is gone from these pages.
- Removed: nothing.

## What changed, checked against the plan

| # | Plan | Status | Where |
|---|---|---|---|
| 1 | Frame, breadcrumb or eyebrow kicker, Edit button, title slot | done; with org "4BC / PROJECT", without (project 1) the eyebrow | `project.rs`, `ui::page_header` |
| 2 | Tabs with count, `COUNT` on Info, rows only on Tasks | done; badge 96 for a project with 96 tasks, 0 for an empty one | `Db::count_tasks_for_project`, `pages.rs` |
| 3 | Two-column Info, Settings panel, shared layout | done; Org uses `ui::info` too (no visible change) | `ui/info.rs`, `ui/info.css`, `org.rs` |
| 4 | Tasks table with New task row; empty state | done; empty project shows the row and "No tasks." | `ui::task_table` (`add`), `project.rs` |
| 5 | Empty description shows nothing | done | `ui::info::description` |
| 6 | No `Nav::load` on the default design, one org lookup | done | `pages.rs` |
| 7 | New-task header and titles ("Error: " prefix) | done | `task_new.rs` |
| 8 | Form on shared controls, `task_fields` | done | `task_fields.rs`, `task_new.rs` |
| 9 | Field-level errors | done; bad issue, empty name, unknown tag, bad duration tested (unit test for all six) | `TaskForm::field`, `forms.rs` |
| 10 | Same POST behavior | done; created task 151, 303 to `/task/151`, tags and status saved | `pages.rs` `task_new_save` |
| 11 | `?design=old` on both GETs | done; old POST comes back as the proposal | `pages.rs` |
| 12 | Speed | done | see 2, 6 |
| 13 | Old code | partly (see item 13: still needed by `?design=old` and the task pages) | `pages.rs` |

Checked in a browser (throwaway copy of the database via `XDG_CONFIG_HOME`): project with and without an organization, Info and Tasks tabs, a project with no tasks, New task GET, refused submits (bad issue, empty name, unknown tag) and a successful create. Tab order on the form: skip link, brand, Board, breadcrumb, fields in order, Create task, Cancel, compare switch. axe: 0 violations on every page above and on Org (Info, Tasks), light and dark, 1280px and 320px, and on the refused form; no horizontal scroll.

Not in the plan (found while testing):
- "No tasks." sat tight under the New task row: `.tasks+.empty-line` gets 16px above.
- A refused submit shows the typed text for name, description, prefix and tags, but an unparsable issue, start or duration comes back empty, because the form is refilled from the applied row. The message quotes the bad value. Left for page 7, which refills task edit the same way.

Verified: `cargo fmt --check`, clippy (0 warnings), 193 tests with `web` (new: task errors name their field, a project's task count) and 170 without.

## Review

Five reviewers (accessibility, complexity, composability, DRY, design). Each claim was checked before applying.

| Finding | Result |
|---|---|
| A1 tabs marker in forced colors | fixed. Cause was also a transparent `border-bottom` from the old `.tabs a` rule that forced colors paints under every tab; `border:0` plus a border line for the current one and underlined text (`ui/tabs.css`). Checked with `forcedColors: active` |
| A2 `.desc` overflow at 320px | fixed (`overflow-wrap:anywhere`, `ui/prose.css`) |
| A3 "—" placeholder | fixed: aria-hidden dash plus `.sr` "Not set" |
| A4 required hint | fixed: `required_note()` in org edit and new task |
| A5 autocomplete, inputmode | fixed: Name `no_autofill` (task and org), issue number `numeric` |
| A6 add row inside the table | fixed: `add_button` above the table (`tasks_tab`); empty issue cell reads "No issue" and stays in the row (no `display:none`) |
| A7 two `aria-current` | fixed with one rule (see `web_components.md`); only the temporary compare switch adds a second one now |
| A8 compare switch overlap | checked, nothing to fix: it is fixed at the bottom left and the pages keep their bottom padding; it is temporary |
| C1 one transaction | fixed: `Db::save_task`, used by create and edit; test added |
| C2 variable limit, index | fixed: chunks of 500, test with 1200 names. No index needed: `UNIQUE(project_id, name)` already leads with `project_id` |
| P1 `/org.css` on project | fixed; `org.css` header and comments corrected (it is the report tab only) |
| P2 `trail` builder | fixed in `ui/breadcrumb.rs`, used by project and new task. Org edit keeps its own two crumbs (no org or project in its chain) |
| P3 shared `tasks_tab`, one `Section`, `project_info` | fixed. The old `pages::Tab` is gone (`old_tab` maps Report to Info) |
| P4 `TaskRow` mapping | fixed: `web/task_rows.rs` |
| P5 "Error: " title | fixed in `frame` (`error` flag), used by org edit and new task |
| P6 consts, status options | fixed |
| P7 split info, `labelled_section`, panel id | fixed. Skipped moving `settings_panel`, `settings_fields`, `task_fields` into `ui/`: they depend on `forms.rs` and the models, and `ui/` must not depend upward |
| P8 thin routes, shared save | fixed: `project::load`, `task_new::load` / `submit`, `TaskForm::create` beside `save` |
| D1 `project_tasks_url` | fixed at every site |
| D2 `FormError::new` | fixed (`EditForm::refusal`) |
| D3 old `task_form` etc. | left: goes with pages 6, 7 (task pages) and the final `?design=old` cleanup |
| S1 form grouping | fixed; hints for issue number and branch prefix |
| S2 keep typed text | fixed for the task form (raw strings). Org edit needed nothing: it has no parsed fields, and its typed values already come back |
| S3 empty Tasks tab | fixed: "No tasks." and a "New task" button |
| S4 kicker, Edit button | kept as is until page 5 |

Verified: axe 0 violations on the project (Info, Tasks, empty, with and without organization), new task, Org and Org edit, 1280px and 320px, light and dark, no horizontal scroll; keyboard order; titles; refused submits (bad issue, empty name, unknown tag, bad start, bad duration) keep every typed value and mark the right field; create with and without tags; `?design=old` renders (its own old-design axe issues predate this work). Not fixed, outside scope: a duplicate task name shows the raw database message.

## Update: task search and quick add

The Tasks tab has the same search box and quick-add row as the organization's (see `doc/web_org.md`); the row posts `project` as a hidden field and the browser returns to `/project/{id}?tab=tasks` (with `q` kept). The "New task" button (the full form) stays.

## Update: New task pop-up

The full-page New task form, the "New task" button and the quick-add row are gone: the Tasks table's first row ("+ New task…") opens the New task pop-up over the list (`?tab=tasks&new=task`, `POST /task/new`), with every field the page had. `GET /project/{id}/task/new` redirects to it. See `doc/web_components.md`, "Pop-ups and the "?" syntax help".
