# Project edit redesign (`/project/{id}/edit`)

> Superseded (final cleanup, `doc/web_cleanup.md`): the compare switch and `?design=old` were removed; the pages render the current design only. References below to the old design, the switch or `proposal` (now `screen`) describe how it was built, not how it is.


Scope: the project edit form (GET shows it, POST saves). It is the sibling of the organization edit form (`doc/web_org_edit.md`) and builds on the project page (`doc/web_project.md`) and the shared components in `src/web/ui/` (`doc/web_components.md`). It adds one component shared with Org edit and New task, `web/edit_page.rs` (the edit-page skeleton), and reuses the rest (see Review).

## Plan

### Page
1. Same frame as Org edit (`ui::frame`, no sidebar). The header is `page_header` with the breadcrumb as kicker, `Acme / Project name / Edit` (the organization when there is one, linked; the project linked to its page, which is where Cancel goes; built by `breadcrumb::edit_trail`, not `trail`: an edit page sits under the page it edits, so its entity links there, while a task page sits under the project's Tasks tab), and the title "Edit {name}", as on Org edit. Document title "Edit project - Name - iter", prefixed "Error: " after a refused submit.
2. The form is `ui::form` + `required_note` + controls + `form_actions`, in the old form's order: Name (required, `identifier`) and Description (`edit_page`), Base path (required, `identifier`, hint), Organization (`ui::select`, hint), Settings last (`settings_fields`, the fieldset shared with Org edit), Save / Cancel (back to the project page).
3. **Base path** is labelled with a hint: "The project's directory on disk. `~` is expanded and a relative path is made absolute." (what `Project::check` does; the old form did not say).
4. **Organization** is a `select` with "No organization" and every organization, the current one selected; hint "The organization the project belongs to. Choose No organization for none." It is one `Db::list::<Organization>()` query, which also gives the breadcrumb's organization (resolved by the handler, `project_edit::org_in`), so the page makes no second lookup.
5. The page is one file, `web/project_edit.rs`, like `org_edit.rs`: one component (`proposal`) that fills `edit_page` with its crumbs, Base path, Organization and Settings; no rules of its own.

### Errors
6. As on Org edit: the message in the focused error notice, linking to the field it is about; that field is `aria-invalid` with `aria-describedby`. `ProjectForm::field` names the field for: empty name (Name), a comma in the name (Name), a name already taken (Name), an empty base path (Base path). Other errors stay general.
7. A duplicate name used to show `database error: UNIQUE constraint failed: projects.name`. `Db::insert` / `update` now map a `UNIQUE` violation (SQLite's extended result code, not its message) to `IterError::NameTaken { kind }`, which reads "a project with this name already exists" ("an organization ...", "a task ... in this project"; the CLI shows the same text) and is tied to the Name field by every form's `field`. This is the "raw database message" left open in `doc/web_project.md`.
8. A refused submit keeps what was typed (name, description, base path, chosen organization, settings). The breadcrumb and title use the *stored* name and organization, not what was typed. `submit` clones the stored row before the submission is applied and returns `Refused { row, stored, extra, error }`; the page takes `stored: &Row`, so there is no second read and no per-entity `Was` type.
9. Behavior does not change: same `ProjectForm`, `validate`, `db.update`, 303 to `/project/{id}`.

### Comparing
10. `?design=old` on the GET shows the old form and sidebar with the switch, as on Org edit. The old form posts without the parameter, so a refused old submit comes back as the proposal. The old `project_form` no longer shows an error (a refused POST always renders the proposal).

### Speed
11. GET: project row and the organization list (one query each); `Nav::load` only for `?design=old`. Refused POST: the project row read once (`submit`) and the organization list once.

### Old code
12. Deleted now: the error path of the old `project_form` and the old-shell rendering of a refused `project_save`.
13. Kept, each with its removal trigger:
    - `project_form` (old, GET only), `old_settings_fields`, `field`, `textarea`, `form_actions`, `check` of `layout.rs`/`pages.rs`: `?design=old` of Org edit and Project edit and the old task form (pages 7, 12).
    - `org_crumb`, `tabs`, `row`, `settings`, `task_table`, `Sel`, `Nav`, `shell`, `layout::error_box`: the old Org, Project, Task pages and old forms (pages 6, 7, 12).

### Feature changes
- Added: breadcrumb (organization / project / Edit); hints on Base path and Organization; required marker on Name and Base path; field-level errors for name and base path; a readable duplicate-name error; the compare switch (GET only).
- Changed: same look as Org edit; Settings is one group; "Description (markdown)" becomes "Description"; the sidebar is gone from the page.
- Removed: nothing.

## What changed, checked against the plan

| # | Plan | Status | Where |
|---|---|---|---|
| 1 | Frame, breadcrumb, title, "Error: " title | done; "4BC / ENERGY-CHECK / EDIT", "Edit project - energy-check - iter" | `project_edit.rs` |
| 2 | Form order, required marker | done | `project_edit.rs` |
| 3 | Base path hint | done | `project_edit.rs` |
| 4 | Organization `ui::select`, one list query | done; project with (4BC) and without (Iter) an organization | `project_edit.rs`, `pages.rs` |
| 5 | One file, no new component | done | |
| 6 | Field-level errors | done; empty name, comma, empty base path checked, the notice link focuses the field | `ProjectForm::field` |
| 7 | Readable duplicate name | done; "a project with this name already exists" | `IterError::NameTaken`, `Db::insert` |
| 8 | Typed values kept, stored crumb/title | done; typed organization kept while the crumb stays 4BC | `Refused::stored` |
| 9 | Same POST behavior | done; changed the organization to Beta (303 to `/project/2`, database row 2), restored to 1 | `forms.rs` |
| 10 | `?design=old` on GET | done; sidebar, switch; its POST comes back as the proposal | `pages.rs` |
| 11 | Speed | done: no `Nav::load` by default, one list query | `pages.rs` |
| 12, 13 | Old code | error path of `project_form` and old-shell refusal deleted; the rest stays with its trigger | `pages.rs` |

Checked in a browser (throwaway copy of the database via `XDG_CONFIG_HOME`): GET, save with a changed organization then restore, refused submits (empty name, name taken, comma, empty base path), Cancel, `?design=old`. axe: 0 violations on the edit page for a project with and without an organization and on a refused submit, light and dark, 1280px and 320px; no horizontal scroll. Tab order: skip link, brand, Board, organization, project, Name, Description, Base path, Organization, the three ticks, three settings fields, Save, Cancel, the compare switch.


Verified: `cargo test --features web` (204 tests, new: project errors name their field and a taken name reads as a sentence).

## Review

Five reviewers (accessibility, complexity, composability, DRY, design). Each finding was checked before acting; all were confirmed.

| Finding | Result |
|---|---|
| A1 invalid marker | fixed: `.v2 .control[aria-invalid=true]` has a 2px `--danger` border (padding 7px 11px so nothing shifts; forced colors keeps the width), and the label reads "Fix this" (`field.rs`, `INVALID_CUE`), a cue that is neither colour nor thickness; checked computed border 2px red, 2px black under forced colors |
| A2 empty option | fixed: "No organization"; `ui::field::optional_options(empty, items)` takes the label |
| A3 Name spellcheck etc. | fixed: Name uses `identifier` on every edit page (organization, project and task names are what the CLI addresses them by); `no_autofill` is gone as nothing else used it |
| A4 no-JS alert | fixed, simplest correct: `role="alert"` is always in the markup and the inline script removes it just before `focus()`; with JavaScript it is read once as the focus target, without it the alert role stays (checked with JavaScript off) |
| C1 name_taken | fixed: `IterError::NameTaken { kind }`, mapped in `Db::insert` / `update` from `SQLITE_CONSTRAINT_UNIQUE` (test `a_taken_name_is_a_typed_error`); no message matching. Skipped `org_choices()` (id, name): the list is one query already and the organization rows are small; a second projection would add a method for no measured cost |
| P1 one skeleton | fixed: `web/edit_page.rs` (`edit_page(kind, subject, name, description, crumbs, action, cancel, error, creating, submit_label, child)`) owns frame, header, form, error box, required note, Name, Description and buttons. It lives in `web/`, not `ui/`, because it needs `forms::NAME` / `DESCRIPTION`, and `ui/` depends on nothing above it. Org edit, Project edit and New task use it (`creating` gives the heading "New task" and the button label; `task_fields` lost Name and Description). Task edit (page 7) and Board edit (page 12) will do the same |
| P2 `Was` | fixed: `Refused { row, stored, extra, error }`, `stored` is the pre-apply clone (`Row: Clone`); `ProjectWas`, `label`, `Was` and the `()` impl on the board form are deleted |
| P3 select helpers | fixed: `optional_options`, `id_text` in `ui/field.rs`; the handler resolves the organization for the crumb (`project_edit::org_in`) |
| P4 thin handlers | fixed: `sidebar(db, old)` and `ui::compare::compared(old, here, child)` on Org, Org edit, Project, Project edit and New task (this also stops the Org page loading `Nav` when it is not the old design). Task edit and the task page stay as they are: still old-only, no switch, until page 7 |
| P5 refusal hook | fixed: `field` is the one hook; `refusal` is its default caller, and the project override is gone. `forms::NAME` / `DESCRIPTION` are shared; `SettingsForm::GITHUB` .. `GITHUB_PROJECT` are used by `settings_fields` |
| D1 edit_trail | fixed: `breadcrumb::edit_trail(org, name, page_url)` shares `org_crumb` with `trail`. The project crumb rule is documented in `breadcrumb.rs`: an ancestor links to the page it is the parent of, so task pages link the project's Tasks tab (`trail`) and an edit page links the entity's own page (`edit_trail`) |
| D2 old code | kept as planned; old `settings_fields` renamed `old_settings_fields`; the misplaced doc comment moved from `project_edit` to the old `project_form` |
| S1 duplicate names | fixed for organizations ("an organization with this name already exists") and tasks ("a task with this name already exists in this project"), on the Name field; tests in `forms.rs` and `error.rs`; checked in a browser for the task |
| S2 error text | fixed: "base path cannot be empty" (error, tests) |
| S3 field order | fixed: relation before Settings, Settings last, on org edit (Projects) and project edit (Organization) |
| S4 hints | fixed: Settings text fields, Projects, Tags, Start, Duration, Issue, Branch prefix follow "what it is. Leave empty for X." Default branch says it is needed instead (an empty one breaks `--save`). Skipped reusing the wording in `settings_panel`: the panel shows values, and its dt/dd rows have no place for instructions |
| S5 `--form-gap` | fixed: `.form` defines it; the group's gap and the closer checkbox rows derive from it (`group.css`); stale text in `web_org_edit.md` and `web_components.md` updated |

Checked in a browser (throwaway database copy via `XDG_CONFIG_HOME`): Org edit, Project edit and New task: GET, save (organization changed and restored), refused cases (empty name, duplicate name, empty base path, unknown project, unknown tag), Cancel, the marker and the select label; the Project and Org pages are unchanged; axe 0 violations on all edit pages (GET and refused), the Project and Org pages, at 1280px and 320px, light and dark; no horizontal scroll; keyboard pass (skip link, brand, Board, crumbs, fields in order, Save, Cancel; the notice takes focus after a refusal and Tab leaves it for the first control); forced colors; `?design=old` still renders. Verified: `cargo fmt --check`, `cargo clippy --features web`, `cargo test --features web` (208), `cargo test --no-default-features`.
