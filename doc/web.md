# iter serve: pages

## Shared frame (every page)
- `ui::frame::frame`: skip link, top bar (brand link to Home, Board; the current section has `aria-current`; Home is the brand link), one `main`, the document title "part - part - iter" ("Error: " first on a refused form). One `h1` per page. The stylesheet is `/v2.css` (tokens, themes, every component sheet); a page may add its own (`/org.css`).
- No sidebar and no design switch: the old design, its `?design=old` switch and `/style.css` were removed (`doc/web_cleanup.md`). `?design=` is now just an ignored query parameter.

## `/` Home
- Header: eyebrow "Home", `h1` "Where to next?", a lede; one card per organization (name, first line of its description, its projects), then a card of the projects with no organization. With nothing at all, "No projects yet." and a hint to run `iter init`. Plan and checks: `doc/web_homepage.md`.

## `/org/{id}` Organization (`?tab=tasks`)
- Info tab: description, settings (github, tmux, auto branch, branch template, default branch, github project), project list.
- Tasks tab: tasks of all its projects (task, project, status, issue; "No tasks yet." when empty); the tab shows their count.
- Report tab (`?tab=report&from=&to=`): the `iter org info` report — period presets and a date form, total, contents per project, then each project's tasks and sessions.
- Edit button. Plan and checks: `doc/web_org.md`.

## `/org/{id}/edit` Organization edit (POST saves)
- Breadcrumb (organization / Edit), title "Edit organization".
- Name, description, Settings group (github, tmux, auto branch, branch template, default branch, github project), Projects checklist (a project in another organization reads "(currently in acme)"; ticking it moves it here); error box, focused, with the field it names marked invalid; Save / Cancel.
- Plan and checks: `doc/web_org_edit.md`.

## `/project/{id}` Project (`?tab=tasks`)
- Header: breadcrumb `Organization / Project` (an eyebrow "Project" when it has no organization), name, Edit button.
- Info tab: description, base path, Settings panel (github, tmux, auto branch, branch template, default branch, github project).
- Tasks tab (count on the tab): tasks (task, status, issue) with a "New task" row on top.
- Plan and checks: `doc/web_project.md`.

## `/project/{id}/task/new` New task (POST creates)
- Breadcrumb (organization / project / New task), title "New task".
- Name (required), description, status, tags, a Schedule group (Start, Duration) and a GitHub group (GitHub issue, branch prefix), each with a hint; error box, focused, with the field it names marked invalid; Create task / Cancel (back to the Tasks tab). Success redirects to `/task/{id}`.
- Plan and checks: `doc/web_project.md`.

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
- Day on the left (heading with the date, previous / Today / next links, 24 hour slots, a now line on today); lists "Important, not scheduled" and "Not important, not scheduled" on the right (below the day when narrow).
- Cards: colored left edge, "Urgent" / "Important" chips, time or duration, a status pill for work in progress, a Schedule / Move link.
- Drag a card to an hour or back to a list (`board.js` posts to `/board/{id}/schedule`); without a mouse, Schedule / Move opens a pick mode (`?pick=`) with a "Schedule here" button per hour and Unschedule.
- Plan and checks: `doc/web_board_agenda.md`.

## `/board/{id}/matrix` Board: Eisenhower matrix
- Header: breadcrumb `Boards / Eisenhower matrix`, board name, Edit button, tabs Agenda / Eisenhower matrix / Info.
- Four numbered quadrants on the left (1 Do first: important, urgent; 2 Plan: important, not urgent; 3 Delegate: urgent, not important; 4 Eliminate: neither), each with its name, rule, task count and a top edge in the priority colors; "Not placed" list on the right (below, with a skip link, when narrow). Cards as on the agenda (chips only in Not placed).
- Drag a card to a quadrant or back to Not placed (`board.js` posts to `/board/{id}/matrix`, flags follow the quadrant); without a mouse, Place / Move opens a pick mode (`?pick=`) with "Place here" / "Move here" per quadrant (a placed task is moved, as on the agenda) and Unplace; a form post is answered with a redirect back to the card, and a status line says "Moved X to Plan" (drag too). The agenda says the same ("Moved X to Tue 29 Sep, 10:00").
- Plan and checks: `doc/web_board_matrix.md`.

## `/board/{id}/info` Board: info
- Header as on the agenda: breadcrumb `Boards / Info`, board name, Edit button, tabs. The description, then a Projects section with the project count (`projects_section`, shared with the Org Info tab: name link and path, wrapping when long; "No projects yet." when none), in one readable column (`info_body`).
- Plan and checks: `doc/web_board_info.md`.

## `/board/{id}/edit` Board edit (POST saves)
- Breadcrumb `Boards / board / Edit`, title "Edit board".
- Name (required), description, Projects checklist (fieldset; a project on another board reads "app (currently in work)" and ticking it moves it here; "No projects yet." when there are none); error box, focused, with the field it names marked invalid (duplicate name: "A board with this name already exists."); typed values and ticks kept after a refusal; Save / Cancel (both to the board's Info page). The board row and its projects are saved in one transaction (`Db::save_group`, also used by Org edit and the CLI).
- Plan and checks: `doc/web_board_edit.md`.

## Messages
- Empty lists say "No X yet." (projects, tasks, sessions, boards); the matrix zones say "Nothing to do first." / "Nothing to plan." / "Nothing to delegate." / "Nothing to eliminate."; the agenda lists say "No important tasks waiting." / "No other tasks waiting."; the org report says "No sessions in this period." (a real filter).
- A refused form reads as capitalised sentences with a period ("Start time '2026-9' is not valid. Use 2026-09-29 14:30."). The web layer maps the messages (`FormError::new`, `sentence`); the CLI keeps its own wording. Hints end "Leave empty for none." unless the meaning differs (branch prefix).

## Routes
GET `/`, `/org/{id}` (`?tab=info|tasks|report&from=&to=`), `/org/{id}/edit`, `/project/{id}` (`?tab=`), `/project/{id}/edit`, `/project/{id}/task/new`, `/task/{id}`, `/task/{id}/edit`, `/boards`, `/board/{id}` (`?date=&pick=`), `/board/{id}/matrix` (`?pick=`), `/board/{id}/info`, `/board/{id}/edit`. POST to the four edit pages and New task (save), `/board/{id}/schedule` and `/board/{id}/matrix` (drops). A missing id is a 404 and a bad id a 400, both as a page built like the others (GET and HEAD); a URL no route matches keeps topcoat's bare 404.

## Assets
- `/v2.css` (tokens plus every component sheet, `ui/*.css`), `/org.css` (the report tab only), `/board.js` (drag and drop on the two board pages), `/org.js` (the report's date auto-apply and copy button). Each is compiled in and served with an ETag (a hash of its text) and `Cache-Control: no-cache`: the browser revalidates and gets a 304 with no body.
