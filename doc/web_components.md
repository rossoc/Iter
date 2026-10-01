# Shared components (`src/web/ui/`)

> Superseded (final cleanup, `doc/web_cleanup.md`): the compare switch and `?design=old` were removed; the pages render the current design only. References below to the old design, the switch or `proposal` (now `screen`) describe how it was built, not how it is.

## Current module layout (after batch B of the final review)

`src/web/` is pages on top, components below.

- **`ui/`** holds the components: one `.rs` and one `.css` per feature (`frame`, `page_header`, `tabs`, `breadcrumb`, `button`, `field` (`field`, `textarea`, `select`, `checklist`), `group`, `form`, `form_actions`, `form_error`, `notice`, `panel`, `labelled`, `columns`, `prose`, `count`, `status`, `tag_chips`, `task_card`, `task_table`, `tasks_tab`, `sessions_table`, `project_list`, `group_card`, `empty_line`, `empty_state`, the board pieces `board_root`, `board_empty`, `day_nav`, `hour_grid`, `side_lists`, `unplaced_list`, `folded_list`, `drop_zone`, `pick_bar`, `pick_here`, `jump_link`, `sticky_head`, `matrix_grid`, `quadrant`, and the private `icons`, `ink`). **`ui` depends on no page code and knows no URL, query or route.** It takes plain data (strings, hrefs, small view structs such as `Tab`, `TaskRow`, `CardView`, `ProjectItem`). The only imports from the rest of the crate are vocabulary types: `TaskStatus`, `Priority`, `is_hex_color` and `IterError`. The one route string it repeats, the Board nav href in `ui/frame.rs`, is asserted equal to `url::BOARDS` by a test in `url.rs`. `ui/mod.rs` also serves `/v2.css`.
- **Web layer (`src/web/*.rs`)**: mappers and page assembly.
  - `url.rs`: every URL and `page_url` (no other module concatenates a path).
  - `sections.rs`: `Section` (`?tab=`), `TabQuery`, `section_tabs`, `tabs_for`.
  - `notes.rs`: `first_line`. `project_rows.rs`: `project_items` (with `~` for the home directory), `projects_section`. `task_rows.rs`, `session_lines.rs`, `board_cards.rs`, `crumbs.rs`, `settings_panel.rs`, `settings_fields.rs`, `task_panel.rs`, `task_fields.rs`: the mappings from models to component data.
  - `layout.rs`: the document, `/board.js`. `assets.rs`: ETag and `Cache-Control` for the scripts and sheets. `errors.rs`, `guard.rs`, `blocking.rs`: layers.
- **Pages** own their route, loader and `screen` component. No `pages.rs`. `home.rs` (`/`), `org.rs` (`/org/{id}`, Info and Tasks; also `/org.css` and `/org.js`), `org_report.rs` (the Report tab: period, `load`, `report_view`), `project.rs`, `task.rs`, `boards.rs`, `board_info.rs`.
  - Board pages: `agenda.rs` (route, loader, `Model`, `screen`), `agenda_cards.rs` (`Cards`, `side`), `agenda_day.rs` (the split by day), `agenda_drop.rs` (`POST /board/{id}/schedule`), `agenda_pick.rs` (its pick words), `matrix.rs` (route, POST, `screen`), `quadrants.rs` (the split), `board_page.rs` (`load_board`), `board_header.rs` (`board_header`, `title_parts`), `drop.rs` (the form and transaction both POSTs share), `pick.rs` (`Picked`, `PickLinks`, `PickWords`).
  - Edit pages: `edit.rs` (`EditForm`, `load`, `load_among`, `submit`, `create`), `edit_page.rs` (the skeleton), `org_edit.rs`, `project_edit.rs`, `task_edit.rs`, `task_new.rs` (`NewTask`), `board_edit.rs`, `project_options.rs` (`project_options`, `group_options`, `project_checklist`), `forms/{mod,org,board,project,task}.rs` (what the forms submit).
  - `load.rs`: lookups shared by loaders (`org_of`, `org_in`, `parents_of`, `grouped`, `unfinished`, `tasks_or_count`, `board_of`, `project_of`).
- Page components are named `screen` (`board_edit::screen`, `task::screen`, ...); the per-page handlers are `show` (GET) and `save` (POST). `router()` in `mod.rs` folds each module's `register`.

Current state: `ui/compare.rs`, `compare.css`, `Nav`, `shell`, `layout::{field, textarea, check, form_actions}`, the old `pages.rs` helpers and `style.css` are gone; sections below that mention them (compare, `pages::sidebar`, "kept for the old design") are history. `/v2.css` is the only stylesheet of the shared design (`/org.css` adds the report tab's rules); `.v2` scoping stays. Entry points: `ui::frame::frame` (page shell), `web/edit_page.rs` (edit pages), `web/board_page.rs` (`load_board` for the agenda and matrix), `web/mod.rs` (`Id`, the `{id}` path parameter).


Scope: step 0 of the redesign of the remaining pages (`doc/web.md` pages 3-12). Home and Organization already use the "v2" design, but every piece of it lives in `v2.rs`, `org.rs`, `home.rs` and `org.css`, and the old pages have their own copies (`pages.rs`, `layout.rs`, `style.css`). This step extracts the v2 pieces into single-feature components, so each later page only assembles them. Home and Organization are converted with no visible change.

## Final state (batch C)

- **Sheets**: 43 component modules (`ui/*.rs`, `mod.rs` included) and 37 stylesheets (`ui/*.css`), concatenated after `v2.css` into `/v2.css`. There is no `home.css` any more (its rules are component sheets); `org.css` is the only page sheet.
- **`task_table(rows: &[TaskRow], caption)`**: the Project column shows when any row carries a project (the organization page). The add button is `tasks_tab`'s (`new_task`), not a row: `new_in` is gone. Empty: `tasks_tab` says "No tasks yet.".
- **Empty states**: `empty_line(text)` is the quiet line, `--muted` (never `--faint`: it is under 4.5:1 on `--bg-2`); `empty_state(svg, child)` the dashed box. Wording: "No X yet." (`NO_PROJECTS` = "No projects yet." on Home, Boards, Org info, Board info, the checklists); the matrix zones keep "Nothing to do first." and so on.
- **Tokens** (`v2.css`, scoped on `.v2`; `--bg` on `:root`; layout on `html`): colors `--bg --bg-2 --bg-3 --line --line-2 --control-line --fg --muted --faint --accent --accent-tint --on-accent --ok --danger`; `--font --mono --shadow --shadow-lift`; `--r-sm/md/lg/pill`; motion `--ease-out --fast --med --slow`; `--line-w`; `--touch` (44px), `--main-w` (1180px), `--main-pad-max`, `--main-pad`, `--measure` (68ch); layout `--bar-h --sticky-head-h --pickbar-h --sticky-h --stick-top`; component-local `--head-gap --edge-w --q1 --q2 --form-gap --num-w --quad-gap --quad-min --info-aside --info-gap --info-body --label-w --dot --control-h`. No type or spacing scale (`--t-*`, `--s*` were not built).
- **`--faint`** is for decoration only (crumb slash, icons); text is `--muted` (5.1:1 on `--bg`, 4.7:1 on `--bg-2` in light; 5.2 and 5.5 in dark).

## Plan

### Module and stylesheet
1. New module `src/web/ui/`, one file per feature. Each component has its rules in a `.css` file next to it, all scoped under `.v2`. `ui::CSS` concatenates the base sheet `v2.css` (tokens, themes, links, focus, `.i`, `.label`, `.mono`, `.sr`, reduced motion) and every component sheet; `/v2.css` serves it. One request, cached, as before. Pages keep only page-only rules (`org.css`).
2. `v2.rs` goes away: its icons, `frame`, `compare`, `is_old`, `tilde` move into `ui/`, and `register` (the `/v2.css` route) into `ui/mod.rs`.

### Components (API, then the pages that will use it)
Every API below is a topcoat `#[component]`, or a plain function where noted. Parameters marked `#[default]` are optional.

3. **`frame`** (`frame.rs`): top bar (brand, Main nav with Board) and centered `main`. `frame(title: &[&str], #[default] error: bool, #[default] current: &str, #[default] styles: &[&'static str], child)` (writes the `<title>`, see "Board agenda after the review" and the forms notes below). The nav item whose `href` equals `current` gets `aria-current="page"`; `styles` are a page's own extra sheets (none of the board pages needs one). Used by: every page (3-12); boards pass `current: BOARDS`.
4. **`icons`** (`icons.rs`): the inline SVG constants (Lucide, ISC) and `icon(svg)`. Adds `PLUS` (new task row). Later pages add their icons here (arrows for previous/next day, trash if needed), one constant each. Used by: everything that shows an icon.
5. **`tabs`** (`tabs.rs`): the section tabs with the sliding underline. `tabs(label: &str, items: &[Tab])`, with `Tab::new(label, href, current)`, `Tab::link(label, href)` and `.count(n)`. `tabs_for(current, [(section, Tab)])` marks the current entry: `section_tabs` (org, project) and `board_tabs` build their rows with it. Renders `<nav class="tabs" aria-label=label>`, `aria-current="page"` on the current one, optional count badge. The Edit link is not a tab (it is the header button). Used by: 4 project (Info, Tasks), 9-12 boards (Agenda, Matrix, Info; Edit stays in the header), Org (done). Task (6) has one section and no tabs.
6. **`status`** (`status.rs`): the glyph pill. `status(state: TaskStatus)`: the icon (`aria-hidden`) and the word, colored per status (queued muted, work in progress accent, done green). Used by: task tables (2, 4), the org report, task page (6) for the current status, the board cards if they show status.
7. **`task_table`** (`task_table.rs`): `task_table(rows: &[(String, Task)], caption: &str, #[default] project_column: bool, #[default] new_in: Option<i64>)`. `rows` is what `Db::tasks_in` returns. `caption` is the screen-reader caption. `project_column` adds the Project column (organization page); `new_in: Some(project)` adds the placeholder "+ New task" row at the top (project page) and keeps the table when there are no tasks. Empty without `new_in`: "No tasks." Used by: Org (`project_column`), 4 project Tasks tab (`new_in`).
8. **`settings_panel`** (`settings_panel.rs`): the Settings aside for an organization or project. `settings_panel(s: Settings)`: on/off flags with check/minus icons, mono text values, "—" when unset. Used by: Org info (done), 4 project info. Not for edit forms (see `form`).
9. **`empty_state`** (`empty_state.rs`): two shapes of "nothing here". `none(text)` is the quiet one-liner (`No tasks yet.`, `No projects yet.`, `No boards yet.`). `empty_state(icon, child)` is the dashed box with an icon and a hint that may contain `<code>` (Home; `/boards`: run `iter board new`). Used by: Home, Org, 4, 6 ("No sessions."), 8 boards, 9 (empty unscheduled lists), 11.
10. **`breadcrumb`** (`breadcrumb.rs`): `breadcrumb(items: &[Crumb])`, `Crumb::link(label, href)` and `Crumb::here(label)`. Renders `<nav aria-label="Breadcrumb"><ol>`, the last item `aria-current="page"`, separators drawn in CSS (not read out). Used by: 4 project (organization), 6 task (organization / project), 7, 5 and 3 (the edit pages link back to the entity), boards 9-12 (Boards).
11. **`page_header`** (`page_header.rs`): kicker (an eyebrow, or the breadcrumb), `h1`, and the actions (child nodes) on the right. `page_header(title, #[default] kicker: Kicker, child)`; the line under it is the separate `lede`. Used by: every page (Home uses `lede`; edit pages pass the breadcrumb and no actions).
12. **`form`** (`form.rs`): the pieces of an edit form on the v2 look. `form(action, cancel, error: &Option<String>, child)` wraps `<form method="post">` with the error box (`role="alert"`) above and Save / Cancel below; `field(name, label, value, #[default] hint)`, `textarea(name, label, value)`, `checkbox(name, label, on)`, `select(name, label, options: &[(String, String)], current: &str)`. Every control has a real `<label for>` (ids derived from `name`), so axe passes without ARIA. Used by: 3 org edit, 5 project edit, 7 task edit and `/project/{id}/task/new`, 12 board edit (project checkboxes, `checklist`). Page 3 added: `field` / `textarea` take `invalid` (`aria-invalid` plus `aria-describedby` to the error notice, next to the hint), `group(title, child)` (`ui/group.rs`, a `fieldset` with a `.label` legend) and `web/settings_fields.rs` (`settings_fields(s)`, shared with page 5). `error_box` also focuses the notice by script, since `autofocus` alone was unreliable.
13. **`compare`** (`compare.rs`): the Before | Proposal switch, plus the URL helper `page_url(base, &[(key, Option<&str>)])`, which builds `base?k=v&...`, skips `None`, and percent-encodes values. Call sites stop concatenating `here` by hand, and tab links use the same helper. `compare(old: bool, #[into] here: String)` and `is_old(design)` stay as they are. Removed together with the `?design=old` branches in the final cleanup.

### Importing
A component and its module share a name (`ui::frame::frame`), and Rust cannot re-export both under one name, so a page imports a component from its module (`use super::ui::frame::frame;`). Plain types and functions are re-exported from `ui` (`Tab`, `Crumb`, `page_url`, `is_old`, `tilde`, the icon constants).

### What stays out of `ui/`
- The old shell (`layout.rs`, `style.css`) and the old pages' building blocks (`pages.rs`: `tabs`, `Tab`, `row`, `settings`, `task_table`, `settings_fields`, `org_crumb`, forms): the old design of project and task pages still needs them, so they are not deleted here. Each page's plan deletes the ones it makes unused (see below).
- Page-only pieces: the Org info layout, the report (period picker, task sections), the Home card grid.

### Deleting old duplicates
Nothing in `pages.rs` can go yet: the old project page (`/project/{id}`, still old) uses `task_table` (with `new_in`), `settings`, `row`, `Tab` and `tabs`; `/org/{id}?design=old` also uses them. Page 4 (project) converts the project page; after it, the old branches are the only users, and the final cleanup deletes them together with `?design=old`. `settings_fields`, `org_form`, `project_form`, `task_form` and `layout::{field, textarea, check, form_actions, error_box}` go with pages 3, 5 and 7.

### Speed
- No new queries and no extra clones per row: components take borrowed slices; `Tab`s and `Crumb`s are tiny per-request vectors.
- One stylesheet response (concatenated at compile time), unchanged in size apart from a few new rules.
- Org still loads tasks on the Info tab only for the count on the tab (one query; recorded in `web_org.md`).

### Accessibility (every component)
- Landmarks: `header`, `nav` (each with its own `aria-label`), `main`; `aside` with `aria-labelledby`.
- `aria-current="page"` on the current tab and last crumb; icons `aria-hidden`; status is icon plus word; table captions and `scope="col"`.
- Focus ring from the base sheet (`:focus-visible`, 2px accent); contrast at least 4.5:1 (`--faint` is used only on `--bg`, `--muted` on tinted surfaces); transitions off under reduced motion; no horizontal scroll at 320px.

### Feature changes
- Added: `PLUS` icon, `breadcrumb`, `form` (unused until page 3), `page_url`, the "+ New task" row in `task_table`.
- Changed: `/v2.css` now contains the component sheets (the old design still loads it for the switch; all rules are scoped under `.v2` / `.compare`). Home and Org markup is rebuilt from components; pixel output stays the same.
- Removed: `v2.rs`; `org.rs` `status_icon`, `flag`, `text`, `settings_panel`, `task_table`; the component rules in `org.css`, `home.css`.

## Checked against the plan

| # | Plan | Status | Where |
|---|---|---|---|
| 1 | `ui/` module, one file and one `.css` per feature, concatenated into `/v2.css` | ✅ 11 components; `v2.css` keeps tokens, links, focus, `.i`, `.label`, `.mono`, `.sr`, reduced motion; `org.css` and `home.css` keep page-only rules | `src/web/ui/` |
| 2 | `v2.rs` removed | ✅ `register` and `tilde` live in `ui/mod.rs` | `ui/mod.rs` |
| 3 | `frame` | ✅ plus `aria-current` on Board | `ui/frame.rs` |
| 4 | `icons`, `PLUS` added | ✅ | `ui/icons.rs` |
| 5 | `tabs` (`Tab::new(..).count(n)`) | ✅ Org uses it; markup identical | `ui/tabs.rs` |
| 6 | `status` | ✅ used by the task table and the report's task headers | `ui/status.rs` |
| 7 | `task_table` (caption, `project_column`, `new_in`) | ✅ `new_in` row (with `PLUS`) is built but not yet shown by any page; page 4 checks it | `ui/task_table.rs` |
| 8 | `settings_panel` | ✅ | `ui/settings_panel.rs` |
| 9 | `empty_state` (`none`, `empty_state`) | ✅ Home and Org use both | `ui/empty_state.rs` |
| 10 | `breadcrumb` (`Crumb::link`, `Crumb::here`) | ✅ rendered by `/org/{id}/edit`, checked in a browser (axe 0) | `ui/breadcrumb.rs` |
| 11 | `page_header` | ✅ Home (eyebrow, lede) and Org (eyebrow, Edit). Every page wraps the title in `.head`; `.head + .lede` keeps Home's 6px gap | `ui/page_header.rs` |
| 12 | `form` (`form`, `field`, `textarea`, `checkbox`, `select`, `error_box`, `form_actions`) | ✅ rendered by `/org/{id}/edit` (`select` by no page yet); `form` is only the `<form>` wrapper, the page composes the rest | `ui/form.rs`, `ui/field.rs` |
| 13 | `compare` + `page_url` | ✅ Org's `here`, tab and report links use `page_url` (unit test added: skips `None`, encodes) | `ui/compare.rs`, `pages.rs`, `org.rs` |
| - | Delete old duplicates | ⏸ none can go yet (see below) | |

Deleted: `src/web/v2.rs`; from `org.rs` `status_icon`, `flag`, `text`, `settings_panel`, `task_table`; the component rules from `org.css` and `home.css`.

Kept for the page that migrates them (old project page and `?design=old` still use them): `pages.rs` `Tab`, `tabs`, `row`, `settings`, `task_table` (page 4 replaces the project page's use; the last users go with `?design=old`), `settings_fields` and the forms (pages 3, 5, 7), `org_crumb` (pages 4, 6), `layout::{field, textarea, check, form_actions, error_box}` (pages 3, 5, 7, 12).

Also noticed, left for later: the Home card rows and the Org project rows are the same "name + right-aligned path" row written twice (`home.rs` `group_card`, `org.rs` `info`); page 4 or the cleanup can extract a `project_row`. `style.css`'s global `form label` and `button` rules still leak into v2 pages, which is why the new form rules set margin and padding explicitly.

Verified (playwright, real database, server on port 3000):
- `/`, `/org/1`, `/org/1?tab=tasks`, `/org/1?tab=report&from=&to=` at 1280px and 320px, light and dark: 16 full-page screenshots before and after are pixel-identical.
- axe-core: 0 violations on all 16; no horizontal scroll at 320px.
- `?design=old` still renders (Home, both Org tabs); the compare switch keeps the tab (`?tab=tasks` and `?tab=tasks&design=old`).
- `cargo fmt --check`, `cargo clippy --features web` (no issues), `cargo test --features web` (185 passed).

## Selection styles

- Sections (`tabs`): one accent underline under the current tab; hovering or focusing another moves it there.
- Period presets (org report): pills in a track; one raised pill marks the current period and follows hover/focus.
- Main nav (`frame`): the current item gets an accent-tint pill, keyed off `aria-current`.
- Compare switch: `aria-current` too.

## Review

API after the review (supersedes items 3-13 above where they differ): `frame(current, styles, child)` with nav items as data and a skip link; `page_header(title, kicker: Kicker, child = actions)` plus separate `lede`; `button::edit_button(href)`; `.button` / `.button.primary` in `button.css`; `task_table(rows: &[TaskRow], caption, project_column, add: Option<AddRow>)`, the caller shows the empty state; `panel(id, title, rows)` with `flag_row` / `text_row`, and `web/settings_panel.rs` maps `Settings` onto it; `empty_line` and `empty_state` in two modules; `notice` / `error_box`, `field` (with `field_id`), `form`, `form_actions(cancel, submit_label)`; `project_list(projects, compact)`; `ui/url.rs` (`page_url`, `org_url`, `project_url`, `task_url`, `new_task_url`, `edit_url`); one `DesignQuery` in `compare.rs`; `.control` for every text control.

| Finding | Result |
|---|---|
| A1 control border 3:1 | fixed: `--control-line` (light `#857f76`, dark `#737aa2`), used by `.control` incl. the report dates |
| A2 `.count` contrast | fixed: text is `--fg` (13.9:1) |
| A3 links at rest | fixed: `.u` has a 1px `--control-line` underline, the accent one grows to 2px on hover/focus; nav bar and breadcrumb unchanged |
| A4 mobile table semantics | fixed: explicit roles on task table and the sessions table; `overflow-wrap:anywhere` on the name cell; project cell has its own class |
| A5 error box | partly fixed: `id`, `tabindex=-1`, `autofocus`, `role=alert` (checked: focused on load). Skipped `aria-invalid`/`aria-describedby`: errors are one string, not tied to a field |
| A6 focus ring | fixed: real 2px outline, offset 1px (checked in forced-colors) |
| A7 skip link, `#main`, separator | fixed; separator has empty alt text (`content: "/" / ""`) |
| C1 org tab count | fixed: `Db::count_tasks_in`; list loaded only for the Tasks tab |
| C2 project_reports | partly fixed: takes the loaded projects (one `projects_in` per request). Skipped batching `task_reports` (2 queries per project): needs an org-wide sessions query, invasive for the CLI |
| C3 clones | fixed in task_table, tabs, breadcrumb, status (`&'static str`), home, forms |
| P1 url helpers | fixed: `ui/url.rs`, test moved |
| P2 page_header | fixed (see API) |
| P3 task_table | fixed (see API) |
| P4 panel | fixed: unique heading id, generic shell |
| P5 empty_state / form split | fixed; checkbox has id + `label for` |
| P6 frame | fixed; `tilde` lives in `project_list.rs`; no `#[allow(dead_code)]` is left in `ui/` |
| P7 report URL | fixed: `org::report_link` / `report_url` / `report_query_url`. Noise in `pages.rs` (`new_in`, `Task::template` wrapping) kept: it is what `cargo fmt` produces and HEAD was not fmt-clean |
| D1 project_list | fixed; modifier is `compact`, not `card` (the old `style.css` styles `.card`, which broke the layout) |
| D2 URL helpers | fixed for org/project/task links and redirects in home, org, pages, layout, board, forms; board URLs have no helper yet (boards are migrated later) |
| D3 shared th | fixed: `.v2 th` and `.label` share one rule |
| S1 header gap | fixed: `--head-gap` (36px) under header, lede and tabs |
| S2 danger, notice | fixed: `--danger` both themes, one `notice.css`; `.error` sets all its properties so the old `.error` rule does not leak |
| S3 button | fixed: one `.button`, `.primary`, `.copy` reuses it |
| S4 tokens | fixed: `--r-pill`, `--med`, `--slow`; `.label` recipe reused; page-background hexes commented; compare.css notes its hexes are temporary |
| S5 `.control` | fixed |
| S6 breadcrumb | current item `--fg`. Skipped dropping the `here` crumb: it keeps the page name in the trail |
| S7 misc | fixed: duplicate `.none` and `.head h1` rules merged, nav keys off `aria-current`, `web_org.md` corrected. Home cards' empty line is now 15px (was 14px) |

Checked in a browser (real database): `/`, `/org/1` info, tasks, report at 1280px and 320px, light and dark: axe 0 violations, no horizontal scroll; `?design=old` renders; form and breadcrumb on a temporary scratch route (removed): axe 0, error box focused on load. Intended visual changes only: resting link underlines, control borders, `Copy summary` now a `.button`, header spacing.

### Form components after page 3 (supersedes the form items above)

- `frame(title: &[&str], ...)` writes the `<title>` ("part - part - iter"; `document_title`).
- `form(action, child)`: the wrapper. Children: `error_box(error: Option<&FormError>)`, controls, `form_actions(cancel, submit_label = "Save")`.
- `field` / `textarea` / `select` take `hint`, `error: Option<&FormError>` (they work out `aria-invalid` and the link to the notice from their own `name`) and `required` (`aria-required` plus a `*`); `field` also takes `identifier` (no spellcheck, capitalization or autofill).
- `FormError { message, field }` (`ui/form_error.rs`): `field` is one of the page's own name constants (`OrgForm::NAME`); `error_box` links the message to that control.
- `group` renders `.fieldgroup` (`.group` is Home's card).
- URL helpers: `org_edit_url`, `project_edit_url`, `task_edit_url`, `tasks_tab_url`.
- Spacing tokens (`--s*`) are not introduced: Home and Org use literal pixels, and a token set for three pages would be a second convention.

### Components after page 4 (supersedes the items above where they differ)

- **Tabs**: `ui::Section` (`Info | Tasks | Report`, `Section::of(query)`, `.title(page)`) and `tabs::section_tabs(base, current, task_count, report_url: Option<String>)` build the tabs of Org and Project; the old design uses `Section` too. `aria-current="page"` rule: a page with tabs marks its current tab; its own crumb is plain text (`Crumb::label`); a page without tabs marks its last crumb (`Crumb::here`). Forced colors: the current tab is underlined text and the line is a border.
- **Breadcrumb**: `breadcrumb::trail(org, project: Option<&Project>, here: Crumb)` builds the chain (project linked to its Tasks tab, `project_tasks_url`).
- **`tasks_tab(rows, caption, #[default] add: Option<String>)`** (`tasks_tab.rs`): the table, or "No tasks." when empty, with the `add_button` ("New task", `button.rs`) above the table (under "No tasks." when empty). Replaces the add row of `task_table` (`AddRow`, `new_in` are gone: the row was a fake data row for screen readers). Used by Org (no add) and Project.
- **`task_table(rows, caption)`**: `TaskRow` is plain display fields with `project: Option<ProjectLink>`; the Project column shows when a row has one. The mapping from models is `web/task_rows.rs` (`project_rows`, `group_rows`). An empty issue reads "No issue".
- **Info body**: `info.rs` is split into `columns.rs` (`info_columns`), `prose.rs` (`description`, wraps long words) and `labelled.rs` (`labelled_section(id, label, child)`, `mono_value`). Org's Projects section and Project's Base path use `labelled_section`. `settings_panel(s, #[default("settings-title")] id)`; an unset text row reads "Not set".
- **Forms**: `required_note()` ("* Required", `form.rs`) above required fields (org edit, new task). `field` gains `no_autofill` and `numeric` (`inputmode`). `FormError::new(&IterError, field)`; `EditForm::refusal(error)` builds it for edit and create. `frame(..., #[default] error: bool)` puts "Error: " in front of the document title (one place for every form).
- **Task inputs** (`task_fields(form: &TaskForm, error)`): Name, Description, Status, Tags loose; groups "Schedule" (Start, Duration) and "GitHub" (Issue number, Branch prefix). The inputs show the form's raw text (`TaskForm::of(task, tags)` on GET, the submitted form after a refusal), so a refused submit keeps everything typed. `TaskStatus::options()`, `TaskForm::DESCRIPTION` / `BRANCH_PREFIX`, `OrgForm::DESCRIPTION` replace literals.
- **Write path**: `TaskForm::create(db, template)` (new) and `EditForm::save` (edit) both end in `Db::save_task(id: Option, task, tag_ids)`: one transaction, no tag write for a new task without tags. `Db::ids_by_name` binds at most 500 names per query.
- **Loaders**: `project::load` / `project::org_of` and `task_new::load` / `submit` keep the routes thin (page 6 and 7 reuse them). `ui::project_tasks_url(id)`.

### Edit pages after the page 5 review (supersedes the form items above where they differ)

- **`edit_page`** (`web/edit_page.rs`, next to the pages because it uses `forms::NAME` / `DESCRIPTION`): the skeleton of an edit or create page. `edit_page(kind, subject, name, description, crumbs, action, cancel, error, creating, submit_label, child)`. It owns the heading ("Edit {subject}", or "New {kind}" when `creating`), the document title parts, `frame`, `page_header` with the crumbs, `form`, `error_box`, `required_note`, the Name (`identifier`) and Description controls and `form_actions`. The page supplies the crumbs (`breadcrumb::edit_trail`, or `trail`) and, as child nodes, the controls between Description and the buttons. Field order rule for every edit page: Name, Description, the entity's own fields, relation fields, Settings last. Used by Org edit, Project edit, New task; Task edit (7) and Board edit (12) do the same.
- **Edit machinery** (`web/edit.rs`): `EditForm::field` is the one hook a form implements for refusals (`refusal` builds the `FormError` from it); `Refused { row, stored, extra, error }` carries the pre-apply row (`Row: Clone`) for crumbs and title. `IterError::NameTaken { kind }` (from `Db::insert` / `update`, by SQLite's result code) is tied to Name by every form.
- **Controls**: `field` marks a control the error is about with a 2px danger border and "Fix this" beside its label; `ui::field::optional_options(empty, items)` and `id_text(id)` build a select that may have no choice. `error_box` is always `role="alert"`; its script drops the role before focusing, so JavaScript-less browsers still announce it and the others read it once.
- **Breadcrumb**: `edit_trail(org, name, page_url)`, `trail` and `org_crumb` share one rule: an ancestor links to the page it is the parent of (task pages: the project's Tasks tab; edit pages: the entity's own page).
- **Compare**: `compare::compared(old, here, child)` wraps a page and its switch; `pages::sidebar(db, old)` loads the old design's sidebar only for it.
- **Spacing**: `.form` defines `--form-gap`; the group's gap and its closer checkbox rows derive from it.
- **Hints** read "what it is. Leave empty for X." (Settings, Projects, Tags, Start, Duration, Issue number, Branch prefix).

### After the page 6 review (supersedes the items above where they differ)

- **Chips**: `ui::tag_chips::tag_chips(chips: &[Chip])`, `Chip { name, color }` (plain display data; `web/task_rows.rs` `chips` maps `Tag`s). The ink logic is `ui/ink.rs` (`readable_ink`, `chip_style`, tests). Chip rules only wrap and color; where the list sits is the container's (`.panel .chips` right-aligns). `.tag` is 2px 10px like the status pill.
- **Panel**: rows `flag_row`, `text_row`, `chips_row`, `value_row`; empty text and empty chips both read "Not set". `web/task_panel.rs` maps a `Task` and its tags. The panel is static, not sticky, under 900px wide or 600px high.
- **Labelled section**: `labelled_section(id, label, #[default] count: Option<usize>, child)`; the badge is `count.css`, shared with the tabs.
- **Sessions table**: the live dot breathes three times and never dims under 0.6; notes keep line breaks (`pre-line`); the report variant's cue is the dot plus "(still running)" for screen readers (no `title`).
- **`ui/` takes no models** (except the plain `TaskStatus` enum in `status`): the breadcrumb chain is built in `web/crumbs.rs` (`trail`, and `edit_trail(org: &Option<Organization>, project: Option<&Project>, name, page_url)` = `trail` + Edit, for the org, project and task edit pages), the project list takes `ProjectItem`s (`web/project_rows.rs`), the sessions table takes `SessionLine`s (`web/session_lines.rs`).
- **Shared lookups**: `web/load.rs` `org_of`, `project_of`, and `Parents { project, org }` with `parents_of(db, project_id)` (404 when missing): the project and task pages, New task and Task edit hold a `Parents`. Shared text rules: `models::end_text`, `Session::duration_text`, `reporting::non_empty`, `task_rows::issue_label`.

### After the page 7 review (supersedes the items above where they differ)

- **`field::checklist(name, label, options: &[CheckOption], hint, empty, error)`**: a checkbox group for a choice of several (each box posts `name=value`; ids `f-{name}-{i}`). A `CheckOption` is `{value, label, note, on}`: the note ("(currently in X)") is secondary text inside the label, so it is part of the accessible name; `on` is decided by the caller, once, so the component does no lookups. It is a `group` (fieldset, legend, then the hint, then the boxes) that takes `id`, `hint`, `described` and `invalid`, so the fieldset is described by the hint and the error notice and shows the cue under the legend. With no options it shows `empty` (`empty_line`). Rows are `min-height` + padding, the box on the first line, a long unbroken name wraps. Used by Board edit and Org edit, whose options are built by `web/project_options.rs` (`project_options::<G>`: one builder for boards and organizations, from `Db::project_owners`). Checked with axe (0 violations, light/dark, 1280/320).
- **`field::invalid_cue`**: the "Fix this" text, now beside the label (in `.field-head`), not inside it, and `aria-hidden`: the control's accessible name stays its label, and a screen reader gets `aria-invalid` plus the error notice in `aria-describedby`.
- **Group legend**: `.fieldgroup legend` is .8125rem (the `.label` recipe, muted 4.5:1+), a step above a panel heading because it names controls.
- **Edit pages share one hook path**: `edit::submit` (edit) and `edit::create` (new) both go through `settle` (apply, then `EditForm::save` or `EditForm::insert`, refusal by `EditForm::refusal`); their closures return `topcoat::Result`. `TaskForm::insert` writes the task and its tags in one transaction.
- **Task pages' arguments**: `task_edit::proposal` and `task_new::proposal` take `(stored: &Task, parents: &Parents, values: &TaskForm, error)`; `task_edit::values(db, &task)` and `task_new::blank(&template)` give the first values.

### Boards page additions
- `group_card` / `card_grid` (`ui/group_card.*`): the overview card and its grid (Home, `/boards`); `NO_PROJECTS` is the shared empty text. The lift and the arrow nudge apply only under `prefers-reduced-motion:no-preference`.
- `prose::first_line(text)`: first line of markdown notes (was `summary`).
- `web/board_header.rs`: `board_heading(board, here)` (crumbs, name, Edit; page 12 uses it alone) and `board_header(board, section)` (heading + tabs).
- `web/load.rs`: `grouped::<G>(db)` gives groups with their projects and the loose projects (used by `/boards` and the sidebar `Nav::load`).

### Board agenda after the review (supersedes the list below where it differs)

Shared by the agenda and the matrix (page 10): `task_card`, `drop_zone`, `unplaced_list` (its "not placed" list, `target: "left"`), `pick_bar` + `pick_here` + `web/pick.rs`, `board.js`, and after the matrix review `board_root(note)`, `board_empty`, `web/board_page.rs`, `web/drop.rs`, `board_cards::CardCtx`.

- **`task_card(card: CardView, #[default] draggable)`**: an `<article id="card-{id}" tabindex="-1" aria-labelledby=title>`; `card_id(id)` is the id a link (`#card-7`) or a focus lands on. `draggable` adds `data-task`; the script does the rest (no `draggable` attribute in the markup, so a page without script shows no grab cursor). The picked card is `aria-current`, has a dashed accent border, a `--bg-3` fill and the words "Being moved".
- **`drop_zone(target, #[default] class, child)`**: `class` is the variant (`drop-list`: a list drawn as a box). Dashed borders (empty hours, lists) appear only under `.can-drag`, which `board.js` sets. `.over` is also an outline in forced colors.
- **`unplaced_list(id, title, empty, count, #[default] target, child)`**: a titled list in a drop zone, "empty" line and a count badge with "(N tasks)" for screen readers (`count_badge(n, noun)`, `labelled_section` takes `noun` too).
- **`folded_list(id, title, count, #[default] open, child)`**: a `<details>` list, not a drop zone (the agenda's "Scheduled on other days").
- **`side_lists(id, label, child)`**, **`sticky_head(child)`** (the day heading and, when picking, the banner, sticky as one), **`jump_link(target, child)`** (visible when focused, and on narrow screens), **`day_nav(id, ...)`** (the h2's id labels the hours: `hour_grid(label_id)`).
- **`pick_bar(verb, title, #[default] state, prompt, cancel, child)`**: `verb` (Scheduling / Moving / Placing) sets `data-mode` (its lower case); only `moving` has an accent of its own (the `--ok` edge), the others use the notice's default. The banner is named by its message (`aria-labelledby`), `PICK_ANCHOR` (`"pick"`) is its id; **`pick_here(action, fields, verb, at)`**: the "Schedule here" button. `web/pick.rs` (what both boards share): `Picked { id, title, start, placed, priority }` (the facts each page words "where it is" with), `picked_in`, `pick_fields(task, target, back)` (the form's names: `task_id`, `target`, `date` or `form`), `pick_link(base, task)` (lands on `#pick`), `card_link`, `moved_link` (`?moved=<task>#card-<task>`), `moved_note` ("Moved X to Y"), `pick_title` ("Placing X" in the document title). `web/agenda_pick.rs`: the agenda's words (`Picked::verb` / `action` / `state`) and `pick_url`, `day_url`, `cancel_url`, `moved_url`; the matrix's words are in `matrix.rs`.
- **Layout tokens** (`v2.css`, on `html`): `--bar-h`, `--sticky-h` (what a page keeps sticky under the bar; 0 when narrow or short), `--stick-top`; `scroll-padding-top` is computed from them.
- **URLs**: `board_day_url(id, day)` (`ui/url.rs`); both endpoints take one `drop::Drop` form (`task_id`, `target`, and optional `date` (agenda) / `form` (matrix), read only by the endpoint that needs it; a `#[serde(flatten)]` of a shared part fails on urlencoded numbers, tested). A form post is answered with a `303` to the page, on the card and with `?moved=<task>` (`agenda.rs` `schedule_back`, `matrix.rs`, `drop::back_or`); the move and the flags are stored in one transaction (`Db::update_placed`).
- **Data**: `Db::cards_unfinished`, `Db::priority_colors`, `load::unfinished`, `models::is_hex_color` (one validator for tag colors, chips and the board's style).

#### The `board.js` contract
`[data-post]` the board root (POST URL; gets `.can-drag`); `[data-task]` a draggable card (its task id; the card's own id is `card-<task>`); `[data-target]` a drop zone (what is posted as `target`); `.dragging` on the card, `.over` on the zone; `.board-notes` (a `role="status"` region, made by `board_root`) where the page says a move was done (`board_root(note)`: "Moved X to Y"; the script writes it again after load so a screen reader reads it, and takes `moved` out of the URL) and where a failed drop is said (`role="alert"`, made if missing). A drop that worked loads the same page with `?moved=<task>#card-<task>` (what a pick form's redirect does too): the fragment puts the focus on the moved card; one that failed says so and does not reload.

### Board agenda additions
- `task_card` (`CardView`, `Edge`, `CardAction`), `drop_zone`, `board_root` (`data-post` and the priority colors), `day_nav`, `hour_grid` / `hour_slot`, `side_lists`, `pick_bar`, `button::post_button` (a form with hidden fields and one button). `web/board_cards.rs` maps `Card`s (`CardCtx { colors, picked }` with `view` / `view_chips(card, label, href, Chips)`, `card_view`, `Colors`, `unfinished`, `time_text`); `web/agenda_day.rs` splits a day (`split`). `board.js` finds boards, cards and zones by `data-post`, `data-task` and `data-target`, so the matrix reuses card, zone and script. Plan: `doc/web_board_agenda.md`.

### Board matrix additions
- `quadrant` (`ui/quadrant.*`): a numbered, named, counted section holding a drop zone; its top edge takes `Edge::modifier()`, the classes of `ui/edge.css` (`--edge-w`, `--q1`, `--q2`, shared with `task_card`; `Edge::of(Priority)` is the one mapping). `matrix_grid` (`ui/matrix_grid.*`, class `quad-grid`): the 2x2 wrap (1 column when narrower than about 500px, reading order 1-4; it owns the quadrants' `flex`). Tokens: `--num-w`, `--quad-gap`, `--quad-min`.
- `web/quadrants.rs`: `QUADRANTS` (Do first, Plan, Delegate, Eliminate), `quadrant_index` / `flags_at`, `split` into `Matrix { waiting, placed }`, `place`. `web/matrix.rs`: page and `POST /board/{id}/matrix` (`form` marker gives a 303 via `drop::back_or`). `web/board_page.rs`: `load_board` / `BoardPage<D>` (agenda and matrix); `ui/board_empty`; `ui/board_root(note)` also loads the script; `unplaced_list(alone)` drops its landmark when the aside is the labelled one. Plan: `doc/web_board_matrix.md`.

### Board info additions
- `web/project_rows.rs`: `project_items` (the one place a project's link is built) and `projects_section(projects, empty)`: the "Projects" `labelled_section` with the count as a bare-number badge (the heading already names the noun) over `project_list` or `empty_line(empty)`. Board info passes "No projects on this board.", Org info "No projects.".
- `ui/columns.rs`: `info_body(child)`, a body with no aside, at the width the body has beside one (`--info-body`, derived from `--info-aside` / `--info-gap` in `columns.css`, so it stays in step with `.info`).
- `count_badge(n, noun)`: the screen-reader text is singular for 1 ("1 task").
- `project_list`: a row is the name with its whole path under it, left-aligned. Nothing is cut (no ellipsis, no `direction:rtl` / `<bdi>`): a long path wraps, preferably after a `/` (`<wbr>`), and a long name breaks anywhere. The last path segment is the strong one (`.leaf`, `--fg`, weight 600) and the directories before it are muted, because the last segment is what tells projects apart. The same layout is used in cards (`compact`: dashed separators, tighter) and on the Info tabs. `split_leaf` (with tests) splits the path.
- `.tabs` wrap (`flex-wrap`); `.count` has a transparent border that forced colors paint.

### Cards and agenda after the list/footer/agenda round (supersedes the items above where they differ)

- **`task_card`**: no flag chips and no `flags` in `CardView`; the priority is the colored edge plus a hidden phrase (`Edge::words`: "Urgent, important"). A card shows no time (the hour row it sits in says it): `time` is only the date of an overdue card ("Tue 29 Sep"), else empty. The own link (`.tcard-act`) is hidden until hover/focus on a big screen with a mouse (`opacity`, so Tab still reaches it); always visible on small screens and touch. A card takes focus without an outline when a link lands on it (`#card-7`).
- **`CardCtx::view(card, label, href)`** is the one builder of a card (agenda and matrix); `Chips`, `view_chips`, `card_view` and `duration_label` are gone. `CardCtx` holds only the picked task.
- **`day_nav(id, title, iso, action, #[default] pick, prev, next, today, is_today, child)`**: the title is the label of a `type=date` field in a GET form to `action` (with hidden `pick`); `board.js` opens `showPicker()` on click and submits on change; without script the field is reachable and a hidden button submits. The child nodes are extra buttons beside the day links (the agenda puts the Not scheduled toggle there). The Today button is tinted, not underlined.
- **`pick_here(..., #[default] slot)`**: `slot: true` (the agenda) is a full-width, 56px, dashed accent box "Schedule here · 11:00" (`pick_here.css`); the matrix keeps the small button. `post_button` takes `#[default] class`.
- **`drop_zone`**: an empty hour no longer draws a dashed box; lists and quadrants keep theirs.
- **Hours** lay their cards in a row of fixed width (`--card-w`, 15rem; full width under 600px).
- **Not scheduled column**: `#side-toggle` (`hidden` until `board.js` runs) toggles `#not-scheduled`, the `.info.side-off` grid and the skip link; remembered in `localStorage["iter.agenda.side"]`.

### Lists: search, quick add (supersedes the items above where they differ)

- **`filter_bar(search: &Search)`** (`ui/filter_bar.rs`): `<form role="search" method="get">` with an optional hidden `tab`, a `type=search` field named `q`, a Filter button, a Clear link (`Search.clear`) and a `<details>` of the syntax (`Search.syntax`, `(term, meaning)` pairs). `Search` holds plain strings; `task_rows::search` and `project_rows::project_search` build it.
- **`task_table(rows, caption, #[default] quick: Option<QuickAdd>)`**: `QuickAdd { action, fields (hidden), projects }` adds the first row; with `projects` the table has the Project column and the row a `<select>`.
- **`tasks_tab(rows, caption, total, search, #[default] add, #[default] quick)`**: the search box (when there are tasks), "N of M tasks match", the table, or "No tasks yet." / "No tasks match."
- **`project_create(create: &ProjectCreate)`** (`ui/project_list.rs`): name and base path fields, an Add project button and a "Browse folders" button (a GET submit to `create.browse` with `home`, name and path), with the refusal of the last try (`error_box`) and what was typed. `projects_section(projects, empty, #[default] search, #[default] create)`.
- **`folder_list(up, dirs, none)`** (`ui/folder_list.rs`): subfolders as links, the way up first; used by `web/folders.rs`.
- **Nav**: `NAV` has Board and Settings (`/settings`, sliders icon).
- `edit::create_to(form, start, extra, land)` is `create` landing where `land(id)` says (the list a row was added from).

### Footer

**`site_footer(data: &FooterData)`** (`ui/site_footer.rs`, `.css` beside it): `FooterData { orgs: Vec<FooterGroup { link, projects }>, boards, loose, settings_href, version }` (the Settings column links to `/settings`), all plain strings. The data comes from `web/footer.rs::load`; `site_footer_for()` (a component with no arguments) opens the database and renders, or renders nothing on an error. Called by `web/layout.rs`, after the page.

### Pop-ups and the "?" syntax help (supersedes "Lists: search, quick add" where it differs)

- **`modal(id, title, close, child)`** (`ui/modal.rs`, `.css` beside it): a pop-up with no script. A page opens one by linking to itself with a flag (`?new=task`, `?new=project`, keeping `tab` and `q`); the server renders the page and the modal over it. Fixed over the viewport; the backdrop blurs the page (`backdrop-filter`) and is itself a link to `close`, as are the ✕ and the form's Cancel. A full-screen sheet under 600px; fade/scale-in only without reduced motion. No Esc (that would need script).
- **`frame(..., #[default] dialog: Option<Child>)`**: the pop-up to render; the top bar and `<main>` then get `inert` (focus, clicks and screen readers stay in the pop-up). A page passes `dialog: x.map(|c| Child::new(view! { some_modal(create: c) }))`. The site footer (layout, outside `.v2`) is not inert, but the fixed backdrop covers it.
- **`field(..., #[default] autofocus)`**: for the first control of a pop-up (not set when the pop-up comes back refused: the error box takes the focus).
- **`task_table(rows, caption, #[default] add: Option<String>)`**: `add` makes the first row a link over the whole row ("+ New task…", a stretched `::after`) to the New task pop-up. `QuickAdd` is gone.
- **`tasks_tab(rows, caption, total, search, #[default] add)`**: `add` goes to the table; the separate "New task" button and `quick` are gone.
- **`project_list(items, #[default] compact, #[default] add)`**: `add` makes the first row "+ New project…". `projects_section(projects, empty, #[default] search, #[default] add)`. The inline `project_create` row is gone.
- **`web/task_new.rs`**: `TaskHome { Project, Org }` (`list_url(q)`, `open_url(q)`), `TaskCreate::in_project` / `in_org` (an organization's asks for the project; none without projects), `task_modal(create)`, and `POST /task/new` (the `TaskForm` plus `project`, `org`, `q`): done, back on the list; refused, the list again with the pop-up, the error and what was typed. `GET /project/{id}/task/new` redirects to the pop-up; `/task/quick` is gone.
- **`web/project_new.rs`**: `ProjectCreate { home, name, description, base_path, settings, error }` and `project_modal(create)` (Name, Description, Base path, Browse folders, Settings). `Home::open_url()` / `open_url_with(name, base_path)` / `create(name, base_path)`. The form's first submit button is a hidden Create project, so Enter never means Browse folders. The folder browser's "Use this folder" is a GET back to the page with `new=project`, `name` and `base_path`; its Cancel goes back to the pop-up. What was typed in Description and Settings before browsing is not carried through the folder browser.
- **`filter_bar`**: the syntax is a round "?" beside Filter (`.filter-help`), not a `<details>`. Hovering shows the tip (`.filter-tip`, the button's `aria-describedby`); clicking focuses the button, so `:focus-within` keeps it shown until a click elsewhere takes the focus. The button has `tabindex="0"` (Safari focuses a button on click only with one) and the tip `tabindex="-1"` (a click inside keeps it open). Under 600px the tip spans the search bar.
- **`project_list` rows** (supersedes "Board info additions" where it differs): the path is on the name's line, never wraps, and loses its start when it does not fit (`.path` is `direction:rtl` with `text-overflow:ellipsis`; `.path-in` isolates the path in its own order, so `~` and `/` stay put); `title` holds the whole path. The name takes at most 70% of the row. With a mouse the path shows only on hover / focus of its row (`opacity`, so nothing moves); touch screens always show it.
- **`field(..., #[default] beside: Option<Child>)`**: a control on the input's line, after it (`.field-row`); New project puts Choose there.
- **Choose (New project)**: a submit button with `formaction="/folders/pick"` (POST, so `guard.rs` refuses it from other sites). `project_new::pick_folder` runs `folder_picker::pick(start)` (blocking; `start` is `folders::start_folder` of the typed path) and renders the pop-up again through `list_page(home, create)` (shared with the refused-create routes), so Description and Settings survive. `Picked::Unavailable` redirects to `/folders`. The inline "Browse folders" button is gone.
- **`filter_bar`** (supersedes the items above where they differ): the "?" is always there (`Search.syntax` must not be empty: `project_rows::project_search` has its own). The form is `position:relative` and at most `min(100%,40rem)` wide; the tip is anchored to its right edge (`right:0`, `max-width:min(26rem,100%)`), so it grows left over the page and stays on screen at any width.
- **Task search default**: `TaskQuery::of(None)` / `text(None)` are the empty query (every task). `url::tasks_tab_url` spells out `q=is:open` (`task_query::DEFAULT`); `tasks_list_url(base, None)` is the unfiltered list, which is where Clear and an emptied box land.
