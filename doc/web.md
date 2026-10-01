# iter serve: pages

## Shared frame (every page)
- `ui::frame::frame`: skip link, top bar (brand link to Home, Board; the current section has `aria-current`; Home is the brand link), one `main`, the document title "part - part - iter" ("Error: " first on a refused form). One `h1` per page. The stylesheet is `/v2.css` (tokens, themes, every component sheet); a page may add its own (`/org.css`).
- No sidebar and no design switch: the old design, its `?design=old` switch and `/style.css` were removed (`doc/web_cleanup.md`). `?design=` is now just an ignored query parameter.

## `/` Home
- Header: eyebrow "Home", `h1` "Where to next?", a lede; one card per organization (name, first line of its description, its projects), then a card of the projects with no organization. With nothing at all, "No projects yet." and a hint to run `iter init`. Plan and checks: `doc/web_homepage.md`.

## `/org/{id}` Organization (`?tab=tasks`)
- Info tab: description, settings (github, tmux, auto branch, branch template, default branch, github project), project list.
- Tasks tab: tasks of all its projects (task, project, status, issue; "No tasks yet." when empty); the tab shows their count. Filter bar and inline "new task" row: see "Task lists" below (the row has a project select here).
- Report tab (`?tab=report&from=&to=`): the `iter org info` report — period presets and a date form, total, contents per project, then each project's tasks and sessions.
- Edit button. Plan and checks: `doc/web_org.md`.

## `/org/{id}/edit` Organization edit (POST saves)
- Breadcrumb (organization / Edit), title "Edit organization".
- Name, description, Settings group (github, tmux, auto branch, branch template, default branch, github project), Projects checklist (a project in another organization reads "(currently in acme)"; ticking it moves it here); error box, focused, with the field it names marked invalid; Save / Cancel.
- Plan and checks: `doc/web_org_edit.md`.

## `/project/{id}` Project (`?tab=tasks`)
- Header: breadcrumb `Organization / Project` (an eyebrow "Project" when it has no organization), name, Edit button.
- Info tab: description, base path, Settings panel (github, tmux, auto branch, branch template, default branch, github project).
- Tasks tab (count on the tab): tasks (task, status, issue), filter bar, and a "+ New task…" first row that opens the New task pop-up (see "Task lists" below).
- Plan and checks: `doc/web_project.md`.

## New task pop-up (`?tab=tasks&new=task`, POST `/task/new`)
- A pop-up over an organization's or project's Tasks tab (`ui/modal`, no script; the page behind is blurred and `inert`). Project (organization lists only), name (required), description, status, tags, a Schedule group (Start, Duration) and a GitHub group (GitHub issue, branch prefix), each with a hint; error box, focused, with the field it names marked invalid; Create task / Cancel. Success returns to the list with its search kept; a refusal shows the list with the pop-up, the error and what was typed. ✕, Cancel and a click on the backdrop close it. `GET /project/{id}/task/new` redirects here.

## `/project/{id}/edit` Project edit (POST saves)
- Breadcrumb (organization / project / Edit), title "Edit {name}".
- Name (required), description, base path (required, with a hint), organization select, Settings group; error box, focused, with the field it names marked invalid (name, base path; a taken name reads "A project with this name already exists."); Save / Cancel.
- Plan and checks: `doc/web_project_edit.md`.

## `/task/{id}` Task
- Header: breadcrumb `Organization / Project / Task` (organization only when there is one), name, Edit button.
- Left: description, then Sessions table (start, end or "ongoing", duration, note; "No sessions yet." when empty). Right: Details panel (status, GitHub issue, branch prefix, start, duration, tags as colored chips with readable text).
- Plan and checks: `doc/web_task.md`.

## `/task/{id}/edit` Task edit (POST saves)
- Breadcrumb (organization / project / task / Edit), title "Edit {name}".
- The same fields as New task: name (required), description, status, tags, Schedule (Start, Duration), GitHub (GitHub issue, branch prefix), with hints; error box, focused, with the field it names marked invalid (a taken name reads "A task with this name already exists in this project."); typed values kept after a refusal; Save / Cancel (back to the task).
- Plan and checks: `doc/web_task_edit.md`.

## `/boards` Boards
- Header: eyebrow "Board" (the name of the section, as in the top bar), `h1` "Boards", a lede. One card per board (name linked to the board, first line of its description, its projects with their paths, "No projects yet." when none), in Home's card grid; with no boards, a hint to run `iter board new`.
- The top bar's Board link is highlighted on every board page. Shared for pages 9-12: `board_url` helpers (`ui/url.rs`), the board header with the tabs Agenda / Eisenhower matrix / Info and the Edit button (`web/board_header.rs`, used by the agenda), `load::board_of`.
- Plan and checks: `doc/web_boards.md`.

## `/board/{id}` Board: agenda (`?date=`)
- Header: breadcrumb `Boards / Agenda`, board name, Edit button, tabs Agenda / Eisenhower matrix / Info.
- Day on the left (heading with the date, previous / Today / next links, 24 hour slots, a now line on today; the day title opens a date picker); lists "Overdue" (open tasks that started before today, with their date; first, when there are any), then "Important, not scheduled" and "Not important, not scheduled" on the right, the column hideable with a toggle remembered in `localStorage`. Tasks starting after today are not shown.
- Cards (shared with the matrix): title, colored priority edge (the flags are hidden text, no chips), no tags, duration or time; 15rem wide, several per hour row; status pill for work in progress; Schedule / Move link, hover-only on big screens. No underline on card text; `.tcard:focus{outline:none}` so the border shows after Cancel.
- Drag a card to an hour or back to a list (`board.js` posts to `/board/{id}/schedule`); without a mouse, Schedule / Move opens a pick mode (`?pick=`) drawing a big dashed "Schedule here" slot per hour, plus Unschedule.
- Plan and checks: `doc/web_board_agenda.md`.

## `/board/{id}/matrix` Board: Eisenhower matrix
- Header: breadcrumb `Boards / Eisenhower matrix`, board name, Edit button, tabs Agenda / Eisenhower matrix / Info.
- Four numbered quadrants on the left (1 Do first: important, urgent; 2 Plan: important, not urgent; 3 Delegate: urgent, not important; 4 Eliminate: neither), each with its name, rule, task count and a top edge in the priority colors; "Backlog" list on the right (below, with a skip link, when narrow). Cards as on the agenda (chips only in Backlog).
- Drag a card to a quadrant or back to Backlog (`board.js` posts to `/board/{id}/matrix`, flags follow the quadrant); without a mouse, Place / Move opens a pick mode (`?pick=`) with "Place here" / "Move here" per quadrant (a placed task is moved, as on the agenda) and Unplace; a form post is answered with a redirect back to the card, and a status line says "Moved X to Plan" (drag too). The agenda says the same ("Moved X to Tue 29 Sep, 10:00").
- Plan and checks: `doc/web_board_matrix.md`.

## `/board/{id}/info` Board: info
- Header as on the agenda: breadcrumb `Boards / Info`, board name, Edit button, tabs. The description, then a Projects section with the project count (`projects_section`, shared with the Org Info tab: name link and path, wrapping when long; "No projects yet." when none), in one readable column (`info_body`). The list has a search box and a "+ New project…" first row (see "Project lists" below).
- Plan and checks: `doc/web_board_info.md`.

## `/board/{id}/edit` Board edit (POST saves)
- Breadcrumb `Boards / board / Edit`, title "Edit board".
- Name (required), description, Projects checklist (fieldset; a project on another board reads "app (currently in work)" and ticking it moves it here; "No projects yet." when there are none); error box, focused, with the field it names marked invalid (duplicate name: "A board with this name already exists."); typed values and ticks kept after a refusal; Save / Cancel (both to the board's Info page). The board row and its projects are saved in one transaction (`Db::save_group`, also used by Org edit and the CLI).
- Plan and checks: `doc/web_board_edit.md`.

## Messages
- Empty lists say "No X yet." (projects, tasks, sessions, boards); the matrix zones say "Nothing to do first." / "Nothing to plan." / "Nothing to delegate." / "Nothing to eliminate."; the agenda lists say "No important tasks waiting." / "No other tasks waiting."; the org report says "No sessions in this period." (a real filter).
- A refused form reads as capitalised sentences with a period ("Start time '2026-9' is not valid. Use 2026-09-29 14:30."). The web layer maps the messages (`FormError::new`, `sentence`); the CLI keeps its own wording. Hints end "Leave empty for none." unless the meaning differs (branch prefix).

## Routes
GET `/`, `/org/{id}` (`?tab=info|tasks|report&from=&to=`), `/org/{id}/edit`, `/project/{id}` (`?tab=`), `/project/{id}/edit`, `/project/{id}/task/new` (redirects to the pop-up), `/task/{id}`, `/task/{id}/edit`, `/boards`, `/board/{id}` (`?date=&pick=`), `/board/{id}/matrix` (`?pick=`), `/board/{id}/info`, `/board/{id}/edit`, `/settings` (`?saved=1`), `/folders` (folder browser). POST to the four edit pages and `/settings` (save), `/board/{id}/schedule` and `/board/{id}/matrix` (drops), `/task/new`, `/org/{id}/project/quick`, `/board/{id}/project/quick` (the New task / New project pop-ups). A pop-up opens with `?new=task` (Tasks tab) or `?new=project` (Info; with `name` and `base_path` when the folder browser sends them back). A missing id is a 404 and a bad id a 400, both as a page built like the others (GET and HEAD); a URL no route matches keeps topcoat's bare 404.

## Assets
- `/v2.css` (tokens plus every component sheet, `ui/*.css`), `/org.css` (the report tab only), `/board.js` (drag and drop on the two board pages), `/org.js` (the report's date auto-apply and copy button). Each is compiled in and served with an ETag (a hash of its text) and `Cache-Control: no-cache`: the browser revalidates and gets a 304 with no body.

## Update: the footer

Every page ends with a large footer (`ui/site_footer.rs`, loaded by `web/footer.rs` and rendered by the layout after the page, so it needs no change in `frame`): the wordmark, then columns for the organizations with their projects, the boards, the projects in no organization (only when there are some), a Settings column linking to `/settings` and About (GitHub link, version). Its root is `<footer class="v2 site-foot">` (it sits outside frame's `.v2`), with `min-height:0` and `content-visibility:auto`. It costs three queries (`grouped::<Organization>` and the boards); if they fail the footer is left out. The static 400/404 document (`errors.rs`) has none, and non-page routes (`/v2.css`, `/board.js`) are not wrapped by the layout.

### Task lists (filter bar and quick add)

Org and project Tasks tabs share `ui/filter_bar` and `web/task_query.rs`. The query (`?q=`) is space-separated terms: `is:open` (what the links into a list carry; no `q`, or an empty box, is no filter, and a query with no `is:` term matches every status), `is:done|wip|queue|all`, `tag:name`, `project:name`, `#id`, and free text matched against the task name; a leading `-` negates a term. The syntax is a "?" beside Filter: hover shows it, a click keeps it until a click elsewhere. The first table row ("+ New task…", the whole row is the link) opens the New task pop-up, posting to `POST /task/new` (org lists add a project select; project lists send `project` hidden). The response redirects back to the same list with `q` kept.

### Project lists (Org and Board Info)

The project list has a search box and a "+ New project…" first row that opens the New project pop-up (name, description, base path, settings): `POST /org/{id}/project/quick` (project joins the organization) and `POST /board/{id}/project/quick` (a board's project has no organization and is assigned to the board). The path is typed or picked with Choose, which opens the system folder picker from the server (`POST /folders/pick`, `web/folder_picker.rs`: osascript on macOS, zenity elsewhere) and shows the pop-up again with the folder; where there is no picker, the folder browser `GET /folders` (`web/folders.rs`, `ui/folder_list`) returns the chosen directory (and the name) to the pop-up.

### Settings page (`/settings`, `web/settings.rs`)

In the nav bar and the footer. A plain form for the config file's defaults: editor, pause gap (minutes), status of a new task, and the project defaults (`settings_fields`, the same controls as the organization and project forms). Validation: editor and default branch not empty, pause gap a whole number >= 0, status one of the three; a refusal shows the form again with the typed values and the error on its field. A save reads the file (`Config::load`), applies the form, writes every key with `Config::save` (comments are not kept; the database path is written back as read), replaces the running config (`config::replace`, so the server follows it at once) and redirects to `/settings?saved=1`. `Config`, `TaskDefaults` and `Settings` derive `Serialize` for this.
