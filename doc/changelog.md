# Changelog

## Unreleased

### Added
- Agenda: the side column is now called Unscheduled, and has a fourth list, Backlog, for tasks neither urgent nor important; the list before it ("Urgent, not scheduled") holds the urgent, not important ones. Overdue and all three lists put urgent, then in-progress tasks first. The Matrix's Not placed column is renamed Backlog, folds into a rail too and lists in-progress tasks first.
- Agenda: the Not scheduled column is now called Backlog; its Important and Not important lists fold to their heading like Overdue.
- Agenda Backlog: the "Important" and "Not important" lists each collapse to their heading (`<details>`, open by default, no JS; `unplaced_list` `collapsible`).
- Agenda: the Backlog column (was Not scheduled) folds into a rail on its own edge (chevron to fold, rail to unfold, count and overdue dot, drop on it to unschedule). State is `?side=off` in the URL, server-rendered; replaces the toolbar button, its `localStorage` key and the script (`ui/side_rail`, `agenda_cards.rs`, `agenda_pick::with_side`).
- Task lists (org and project Tasks tabs): filter bar (`is:open` default, `is:done|wip|queue|all`, `tag:`, `project:`, `-` negation, free text, `#id`) in `src/web/task_query.rs`.
- Task lists: inline "new task" first row (`POST /task/quick`; org lists have a project select); returns to the same list.
- Org and board Info project lists: search box and create row (`POST /org/{id}/project/quick`, `POST /board/{id}/project/quick`; a board's project has no organization and is assigned to the board).
- Folder browser `GET /folders` (`src/web/folders.rs`) to pick a project path.
- Global footer on every page (`ui/site_footer`, `web/footer.rs`): organizations, projects, boards, GitHub link, version, link to Settings.
- Editable `/settings` page (`src/web/settings.rs`, nav bar item). `Config::save` writes `iter.yaml` (comments are lost); `config::replace` swaps the running config.
- Agenda: "Not scheduled" column toggle (remembered in `localStorage`); "Overdue" list for past open tasks, first in the column; big dashed "Schedule here" slot in pick mode; the day title opens a date picker.

### Changed
- Task and project lists: the first row is a link ("+ New task…" / "+ New project…") that opens a pop-up over the blurred page with the whole create form (task: project on org lists, description, status, tags, schedule, GitHub; project: description, base path, settings). No JavaScript: the pop-up is `?new=task|project` rendered by the server, the page behind is `inert`, and closing is a link (✕, Cancel, the backdrop). New component `ui/modal`; `frame` takes `dialog`.
- `POST /task/new` replaces `POST /task/quick`; `GET /project/{id}/task/new` redirects to the pop-up (the full-page New task form and the "New task" button are removed). The inline project create row is removed.
- New project pop-up: the base path and a "Choose…" button share one line. Choose posts the pop-up to `POST /folders/pick`, which opens the system's own folder picker (`osascript` `choose folder` on macOS, `zenity` elsewhere; `src/web/folder_picker.rs`) at the typed path (nearest existing folder) or the home folder, and shows the pop-up again with the chosen folder (and its name, when none was typed); everything else typed is kept. Without a picker it falls back to the folder browser (`/folders`), whose "Use this folder" returns to the pop-up instead of creating the project directly.
- Project lists (org/board Info, homepage cards): the path sits on the name's line, and a path too long for the row loses its start (`…/projects/iter-webapp`, CSS right-to-left box with the path isolated inside; the full path is the tooltip). It shows only when its row is hovered or holds focus (always on touch screens); its space stays reserved so nothing moves.
- Filter bar (one component, `ui/filter_bar`, on every list with a search: tasks and projects): the syntax is a "?" beside Filter; hover shows it, a click keeps it shown until a click elsewhere (CSS `:focus-within`, no script). The tip hangs from the bar's right edge and grows to the left, at most the bar's width, so it never leaves the screen. The project search now has a syntax too. The bar is at most 40rem wide.
- Task search: an empty box means no filter (every task, done ones too). The links into a Tasks tab (the tab, breadcrumbs, back links) carry `q=is:open`, so a list still opens on the open tasks; a list with no `q` is unfiltered (the server reads an empty `q=` as none). Clear empties the search. A query with no `is:` term matches every status.
- `Config`, `TaskDefaults` and `Settings` derive `Serialize`.
- Agenda cards are shared with the Matrix: no tags, duration or time; priority edge with hidden text; fixed 15rem width, several per row; Schedule link hover-only on big screens; no underline.
- Agenda hides tasks scheduled after today.

### Fixed
- Card border is visible after Cancel in pick mode (`.tcard:focus{outline:none}`).

### Notes
- Visual and JS behavior of these changes has not been verified in a browser.
