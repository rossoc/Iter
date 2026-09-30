# Board edit redesign (`/board/{id}/edit`)

> Superseded (final cleanup, `doc/web_cleanup.md`): the compare switch and `?design=old` were removed; the pages render the current design only. References below to the old design, the switch or `proposal` (now `screen`) describe how it was built, not how it is.


Scope: the board edit form (GET shows it, POST saves), page 12 and the last one. It is the sibling of Org edit (`doc/web_org_edit.md`), Project edit (`doc/web_project_edit.md`) and Task edit (`doc/web_task_edit.md`), and builds on the board pages (`doc/web_boards.md`, `doc/web_board_info.md`) and the shared components in `src/web/ui/` (`doc/web_components.md`). It adds no page component: `edit_page`, `crumbs`, `ui::field::checklist`, `form_actions` and `notice` already do everything it needs (the review below moved the option builder to `project_options.rs`, shared with Org edit).

## Plan

### Page
1. Same skeleton as the other edit pages: `edit_page` (frame without sidebar, `page_header` with the breadcrumb as kicker, `form`, error notice, required note, Name, Description, the page's own controls, `form_actions`). Only the crumbs and the controls in the middle differ. The frame marks the top bar's Board link as current, as on the other board pages (`edit_page` takes an optional `current`).
2. Header: breadcrumb `Boards / {board} / Edit` (Boards linked to `/boards`, the board to its Info page, which is where Save and Cancel go) and the title "Edit {name}". The crumbs are `crumbs::edit_trail`'s twin for boards: both end with the same "name linked to the page being edited, then Edit" (one private `edit_tail`), and `Boards` is the same crumb `board_heading` uses (`crumbs::boards_crumb`). Document title "Edit board - Name - iter", prefixed "Error: " after a refused submit.
3. Form: Name (required), Description (`edit_page`), then **Projects**, a `ui::field::checklist` (fieldset, legend "Projects", the hint between legend and boxes, one checkbox per project, in name order, all projects). A project that is on another board says so beside its name as secondary text inside the label: "app (currently in work)"; the hint says "Ticking a project moves it here. Unticking one moves it off this board." (the same wording as Org edit). With no projects at all the group says "No projects yet." (`NO_PROJECTS`, via `empty_line`) instead of being empty. Save / Cancel (back to the Info page).
4. Values: the board's checked projects are the ones whose `board_id` is the board; after a refused submit they are the ones ticked in the submission. The submission is read as raw pairs (the repeated `project` field cannot be a struct field), as before: `BoardForm::parse`, moved next to the other forms in `forms.rs`.
5. One file, `web/board_edit.rs`, like `project_edit.rs`: one component `proposal` that fills `edit_page` with its crumbs and the checklist. The options come from `project_options::project_options` (shared with Org edit). No rules of its own, no CSS file of its own (the checklist row and group rules are in `ui/field.css` and `ui/group.css`).

### Errors
6. As on the other edit pages: the message in the focused error notice linking to the field it is about, that field `aria-invalid` with the "Fix this" cue. `EditForm::field` by default names Name (empty, taken, comma). The checklist is given the same error, so a refusal that is about the Projects group (none exists today: an id that names no project changes nothing) marks the fieldset and links to it without a further change.
7. A refused submit keeps everything typed (name, description and the ticked projects), and the page's crumbs and title use the *stored* board (`Refused::stored`), so what was typed never reaches the breadcrumb. A taken name reads "a board with this name already exists" (`IterError::NameTaken`); an empty one "board name cannot be empty".
8. Behavior does not change: same validation, 303 to `/board/{id}/info`. The board row and its projects are now written in one transaction (`Db::save_group`), where before two transactions ran one after the other, so a refused name leaves the projects as they were and a failure between the two cannot leave half a save.

### Comparing
9. `?design=old` on the GET shows the old form and sidebar with the switch (`compared`). The old form posts without the parameter, so a refused old submit comes back as the proposal. The old form (`board::old_edit`) no longer shows an error (a refused POST always renders the proposal).

### Speed
10. GET: the boards and the projects, two queries (the board is found among the boards: no third read by key; a missing one is a 404), and the project to board-name lookup is one pass over a map. No sidebar query (`Nav::load` only for `?design=old`). Refused POST: the board once (`submit`), then the same two lists; nothing is read twice. Save: one transaction.

### Old code
11. Deleted now: the `error` parameter of the old board form, the old `edit_page`/`save_page` routes, `board::register`, `choices`, `Choice` (now `project_options` and `ui::field::CheckOption`), `BoardForm` and its test (moved to `forms.rs`), `layout::error_box` and the bare `.error` rule of `style.css` (nothing but `error_box` used them; the notice's own `.v2 .notice.error` is self-contained).
12. Kept, each with its removal trigger:
    - `board::old_edit` (GET only), `layout::field`, `textarea`, `form_actions`, `check` and the `form label`, `input`, `textarea` rules of `style.css`: `?design=old` of Board edit and the old Org, Project and Task forms (`pages.rs`); the final cleanup pass.
    - `Nav`, `Sel`, `shell`: every `?design=old` page; the final cleanup pass.

### Feature changes
- Added: breadcrumb (Boards / board / Edit); a hint on Projects and the current board as text in the label; required marker; field-level errors; typed values and ticked projects kept after a refusal; a readable duplicate-name error; the compare switch (GET only); one transaction for the save.
- Changed: same look as the other edit pages; the sidebar is gone from the page; the projects are a labelled group (fieldset and legend) and "(now on work)" in a muted colour becomes text in the label, "(currently on work)".
- Removed: nothing.

## What changed, checked against the plan

| # | Plan | Result | Where |
|---|---|---|---|
| 1-3 | One skeleton, crumbs Boards / board / Edit, Projects `ui::field::checklist` | done; "beta (currently in home)" as text, hint under the group, current top bar item | `board_edit.rs`, `crumbs::board_edit_trail` |
| 4 | Values and ticks | done; refused submit keeps typed name, description and ticked projects | `BoardForm::parse` |
| 5 | One file, no component | done | `board_edit.rs` |
| 6-7 | Field errors, stored crumb/title | done; empty name and duplicate name mark Name, focus the notice; title "Error: Edit board - work - iter" | `BoardForm::field` |
| 8 | Same behavior, one transaction | done; 303 to `/board/1/info`; a refused duplicate name left the project unmoved | `Db::save_group` |
| 9 | `?design=old` on GET | done; old form, no sidebar on the proposal | `board::old_edit` |
| 10 | Speed | done: two queries, one map pass, no `Nav::load` by default | `project_options` |
| 11-12 | Old code | deleted as listed; the rest stays with its trigger | `board.rs`, `layout.rs`, `style.css` |

Checked in a browser (throwaway database via `XDG_CONFIG_HOME`): GET, save with a changed name, description and projects (restored), refused cases (empty name, duplicate name; values and ticks kept), Cancel (to `/board/1/info`), `?design=old`, other pages unchanged. axe: 0 violations on GET (light, dark, 320px) and on a refused submit; no horizontal scroll at 320px; reduced motion and forced colors emulated (error cue and checkbox states remain visible); keyboard: skip link first, notice focused after a refusal. Verified: `cargo fmt --check`, `cargo clippy --features web`, `cargo test --features web` (266), `cargo test --no-default-features` (179). New tests: `project_options` (ticks, other group named, submission decides, organizations), `BoardForm`/`OrgForm` parse and field mapping, `save_group` transaction and `assign_projects` (`db.rs`), `board_edit_trail`.

## Review

Five reviewers (accessibility, complexity, composability, DRY, design). Each finding was verified before it was applied.

| Finding | Result |
|---|---|
| A1 long unbroken name at 320px | fixed: `.check-text` has `min-width:0; overflow-wrap:anywhere`, the row `align-items:flex-start`, the box aligned to the first line (`1lh`). Checked with a 100-character name: no horizontal scroll |
| A2 empty checklist | fixed: `checklist` takes `empty`, renders `empty_line` inside the group; both forms pass `NO_PROJECTS` (checked on a database without projects) |
| A3 hint between legend and body | fixed (`group.rs`; `aria-describedby` unchanged). Legend size: skipped, kept as is: `.fieldgroup legend` is one rule for every group (Settings, Schedule, GitHub, Projects), so all groups already match; a non-uppercase legend would set them apart from the `.label` headings of the rest of the design |
| C1 O(1) tick test | fixed: options carry their own `on` (`CheckOption`), built from a `HashSet`; no `contains` on a slice, no string rebuilds |
| C2 `assign_projects` | fixed: two set-based statements touching only changed rows (`... WHERE owner = ?1 AND id NOT IN (...)`, `... WHERE owner IS NOT ?1 AND id IN (...)`); the ids travel as one JSON array through `json_each`, so a roster of any length is one bound variable (no chunking needed, tested with 1200); a repeated or unknown id is harmless; the forms' `parse` also sorts and dedups (`Pairs::ids`) |
| C3 narrow query | fixed: `Db::project_owners` (id, name, owner id), five lines, instead of full `Project` rows |
| P1 `Choice`/`choices` out of `board_edit.rs` | fixed: `project_options.rs`, one builder generic over `ProjectGroup + Table` (owner column, owner names map); `board::old_edit` takes `&[CheckOption]` |
| P2 Org edit uses the checklist | done: `OrgForm::parse` reads repeated `project` fields, `project_options::<Organization>`, comma-separated text field removed; 303 and refusal behaviour kept, typed ticks kept. `names`/`joined`/`ids_by_name` stay: tags and the CLI use them. The org-only "unknown project name" error path is gone (ids cannot be typos). The old org form (`?design=old`) lists the projects as plain checkboxes too (`layout::old_project_checks`, shared with the old board form) |
| P3 docs and `choice_id` | fixed: documented as `ui::field::checklist`; `choice_id` private, no `cfg_attr` |
| D1 `Db::save_group` | fixed: one generic method, used by both forms and by the CLI's `update_group` (now atomic); test covers a board and an organization |
| D2 `Db::atomically` | fixed: replaces the repeated sites (`set_task_tags`, `save_task`, `set_projects`, `save_group`, `update_placed`); `batched` untouched |
| D3 default `EditForm::field` | fixed: default is `name_field` (EmptyName, NameTaken, CommaInName); Project and Task forms fall back on it, `BoardForm`/`OrgForm` have no `field`. Optional shared name+description apply helper: skipped, four lines per form and each form applies its own trimming |
| D4 leftovers | kept, triggers unchanged (see 12); `doc/web_board_matrix.md.bak` deleted; `.playwright-cli/` left for the final pass |
| S1 note style | fixed: `<span class="note">` (muted, .8125rem, weight 400) inside the label, so it is part of the accessible name; hover colour leaves it muted. Wording now "(currently in X)" for both boards and organizations |
| S2 rows | fixed: `min-height` and 6px padding, no negative margins; group CSS derived from `--form-gap`; the `.fieldgroup .hint` rule moved to `group.css`. Checked-row emphasis: skipped (the checkbox is enough). Long list cap (`max-height`, `overflow:auto`): skipped, it clips the focus ring; a long list is accepted and the page scrolls |
| S3 hint wording | fixed: "Ticking a project moves it here. Unticking one moves it off this board/organization." |
| S4 docs | done: this file, `web_components.md`, `web_org_edit.md`, `web.md` |

Checked again after the fixes: board and org edit (GET, save with ticks changed then restored, refused empty and duplicate names with typed values and ticks kept), empty project list, 100-character name at 320px, 1280px and 320px, light and dark, reduced motion, forced colors, axe 0 violations on all of them, keyboard (Tab through the boxes, Space toggles), `?design=old`, other pages 200; `cargo fmt --check`, `cargo clippy --features web`, `cargo test --features web`, `cargo test --no-default-features`.
