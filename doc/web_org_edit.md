# Organization edit redesign (`/org/{id}/edit`)

> Superseded (final cleanup, `doc/web_cleanup.md`): the compare switch and `?design=old` were removed; the pages render the current design only. References below to the old design, the switch or `proposal` (now `screen`) describe how it was built, not how it is.


Scope: the organization edit form (GET shows it, POST saves). It builds on Home and Organization (`doc/web_homepage.md`, `doc/web_org.md`) and on the shared components in `src/web/ui/` (`doc/web_components.md`). It is the first form page, so it also fixes how every later edit page (project, task, board) looks.

## Plan

### Page
1. Same frame as Home and Org (`ui::frame`). The header is `page_header` with the breadcrumb as kicker (`Acme / Edit`, the organization linked back to its page) and the title "Edit {name}" (the stored name; document title "Edit organization - Name - iter"). No action button: the actions are at the end of the form.
2. One column of fields, at most 640px wide (`ui::form`), in this order: Name, Description, Projects (the relation), Settings (always last; the rule for every edit page). The page is `web/edit_page.rs`, shared with the project form and the new-task form; this page supplies its crumbs and Projects. Every control has a real `<label for>`. Fields sit on the darker `--bg-2` surface with the shared focus ring (`.control`).
3. **Settings** is a group: a `fieldset` with a `legend` styled like the Settings panel's heading, holding the three checkboxes and three text fields (Branch template, Default branch, GitHub project). This is the only new component (`ui::group`, one feature: a titled group of controls). The six inputs are shared by the org and project forms in `web/settings_fields.rs`, next to `settings_panel.rs` (the read-only twin), so the two stay in step.
4. **Projects** is the same checklist as Board edit (`ui::field::checklist`, options from `project_options`, after the board-edit review): one checkbox per project, a project in another organization reads "(currently in acme)" as secondary text. The hint says "Ticking a project moves it here. Unticking one moves it off this organization."; with no projects at all it says "No projects yet." The submission carries repeated `project` fields (`OrgForm::parse`). This replaces the comma-separated text field, and with it the "unknown project name" error. The old form (`?design=old`) lists checkboxes too, so it still posts.
5. Save (accent button) and Cancel (link back to the organization) end the form (`form_actions`).

### Errors
6. A rejected submit shows the form again with what was typed, and the message in the error notice above the form (`ui::notice`). The notice takes focus on load (its `role=alert` is dropped by the script just before it takes focus, so it is read once; without JavaScript the role stays and the alert is announced), and when the error is about a field its text links to that field.
7. When the error belongs to one field, that field gets `aria-invalid="true"` and `aria-describedby` pointing at the notice (as well as its hint). One error is tied to a field: an empty or taken name (Name). The link is made by the form (`EditForm::field`, next to `apply`), so project and task forms can use it later. Other errors (database failures) stay general.
8. Behavior does not change: the same `OrgForm`, validation, `save` (roster resolved before anything is written), and the 303 redirect to `/org/{id}`.

### Comparing
9. `?design=old` on the GET shows the old form, with the switch, like the other pages. The switch is a link, so it works for the GET only. The old form posts to `/org/{id}/edit` without the parameter, so a rejected old submit comes back as the proposal. That is acceptable: the old form is a reference, not a way to save, and adding a hidden field or a query to the POST would change its behavior. The old view is deleted with the other `?design=old` branches at the end.

### Speed
10. No new queries on GET: one row and `projects_in`. The proposal has no sidebar, so it skips `Nav::load` (two queries); only `?design=old` loads it. A refused POST reads no other row: the stored row, for the breadcrumb and title, is cloned before the submission is applied (`Refused::stored`).

### Feature changes
- Added: breadcrumb back to the organization; hint on Projects; field-level `aria-invalid` / `aria-describedby` for name and projects errors; the compare switch (GET only).
- Changed: same look as Home and Org (frame, tokens, buttons); Settings are one labelled group; "Description (markdown)" becomes "Description" (the text is shown as plain text with line breaks); the sidebar is gone from the page, as on Home and Org.
- Removed: nothing.

## What changed, checked against the plan

| # | Plan | Status | Where |
|---|---|---|---|
| 1 | Frame, breadcrumb kicker, title | done | `org_edit.rs`, `ui::page_header` |
| 2 | One column, labels, `.control` surface | done | `ui::form`, `ui::field` |
| 3 | Settings as a fieldset (`ui::group`), inputs shared with the project form | done | `ui/group.rs`, `settings_fields.rs` |
| 4 | Hint on Projects, `aria-describedby` | done | `ui::field` |
| 5 | Save / Cancel | done | `ui::form_actions` |
| 6 | Error notice, focused on load | done, with a fix (below) | `ui::notice` |
| 7 | `aria-invalid` and `aria-describedby` for name and projects | done; checked with an empty name and an unknown project | `EditForm::field`, `forms.rs`, `ui::field` |
| 8 | Same POST behavior | done; saved, redirected (303) to `/org/1`, restored | `forms.rs` unchanged apart from `field` |
| 9 | `?design=old` on GET | done; the old form's POST comes back as the proposal | `pages.rs` `org_form` |
| 10 | No sidebar queries | done | `pages.rs` `org_edit` |

Checked in a browser (throwaway copy of the database via `XDG_CONFIG_HOME`): GET, a successful save with keyboard only (description with a line break, a tick, Enter on Save; then restored), an empty name, an unknown project, Cancel. axe: 0 violations on GET and both errors, light and dark, at 1280px and 320px; no horizontal scroll. Tab order: skip link, brand, Board, breadcrumb, the fields in order, Save, Cancel, the compare switch, every stop with a visible focus.

Not in the plan (found while testing):
- A refused submit showed an empty breadcrumb link when the name was empty (`page.row` holds what was typed), which axe flags. The page now takes the stored name separately.
- `autofocus` on the error notice worked in about half of the submits in Chromium. `error_box` now also focuses it with one inline line, which also serves the project and task forms later.
- The old design's `org_form` no longer shows an error (a refused POST always renders the proposal); `layout::error_box` is still used by the other old forms.

Code: `ui::group` (new, one feature), `invalid` on `field` / `textarea`, `EditForm::field` and `FormError` in `edit.rs`. The `#[allow(dead_code)]` on the form modules is gone. The old `settings_fields` in `pages.rs` stays for the project form and `?design=old` until page 5.

Verified: `cargo fmt --check`, clippy (0 warnings), 188 tests pass (new: which errors name a field, `aria-describedby` list).

## Review

Five reviewers (accessibility, complexity, composability, DRY, design). Each claim was checked before acting.

| Finding | Result |
|---|---|
| A1 title always "iter" | fixed: `frame(title: &[..])` writes `<title>` ("Edit organization - Acme - iter", "Error: " prefix on a refused submit); Home ("iter") and Org ("Organization / Tasks / Report - Acme - iter") use it; the old `shell` writes "iter". The layout's `<head>` has no title any more: it renders before the page knows its own, and browsers take the first `<title>` (checked: `document.title`) |
| A2 required Name | fixed: `required` prop (`aria-required`, `*` with `aria-hidden`). Not the HTML `required` attribute, which would replace the server's message with the browser's |
| A3 scroll-padding | fixed: `html:has(.v2){scroll-padding-top:72px}` |
| A4 autocomplete / spellcheck | fixed: `identifier` prop on the branch, project and Projects fields |
| A5 notice | fixed: no `role=alert` and no `autofocus` when it takes focus, hidden "Error: " (`.sr`), thicker left edge (also under forced colors), `overflow-wrap:anywhere` |
| A6 link to the field | fixed: the message links to `#f-<name>`; checked, the click focuses the field |
| A8 forced colors `.u` | fixed: underline under `forced-colors:active` |
| A9 `.group` clash | fixed: `.fieldgroup` |
| C1 `Nav::load` on the default design | fixed: `load` / `submit` no longer load the nav; callers that show the sidebar (old design, project, task, board forms) load it in `extra` |
| C2 second read of the org | fixed: the stored row is cloned before `apply`; `submit` returns `Refused { row, stored, extra, error }` (after the page 5 review; it was `EditForm::label` and `was`) |
| C3 project names | fixed: `ids_by_name` dedupes and resolves in one query (`name IN (...)`); `projects.name` is `UNIQUE`, so already indexed (no migration); the CLI callers are unchanged; tests for repeats and none. `set_projects` keeps its per-project UPDATE |
| C4 clones | fixed: `settings_fields(&Settings)`, no message clone, `here` moved |
| P1 `error_box` | fixed: `notice(focus)` writes the focusing line from its own `id`; `ERROR_ID` is in `notice.rs` |
| P2 `form` wrapper only | fixed: the page composes `error_box`, controls and `form_actions` |
| P3 typed error to field | fixed: `FormError` in `ui/form_error.rs`, `OrgForm::NAME` / `PROJECTS` used by `EditForm::field` and the page, controls derive `invalid` from `Option<&FormError>` |
| P4 hint / invalid on textarea and select | fixed: same props on all three, one `labelled` wrapper and `hint_id` |
| D1 URL helpers | fixed: `tasks_tab_url`, `org_edit_url`, `project_edit_url`, `task_edit_url` replace the hand-built copies |
| D2 old duplicates | left, as planned, until pages 5, 7 and 12 |
| S1 spacing | fixed: group margins, checkbox rows 12px apart, 10px more above the actions. A hairline above the group was skipped: a fieldset border fights its legend |
| S2 title | fixed: "Edit Acme" (as Org's h1 is the name); the crumb stays "Edit" |
| S3 compare on refused POST | fixed |
| S4 stale allows, comments and doc text | fixed |
| S5 `.check` hover | fixed: pointer, accent on hover. `--s*` tokens skipped (Home and Org use literals), see `web_components.md` |
| Projects before Settings | skipped: keeps the old form's order |

Checked in a browser again (throwaway database via `XDG_CONFIG_HOME`): GET, save with Enter, empty name, unknown project, the error link, Cancel, tab order. axe: 0 violations for the edit page (GET and both errors), Home and Org (Info, Tasks, Report) at 1280px and 320px, light and dark, no horizontal scroll. `?design=old` keeps its own old-design violations (form labels, one contrast), which the cleanup pass removes with the design. `cargo fmt --check`, clippy (0 warnings), 191 tests with `web` and 170 without.
