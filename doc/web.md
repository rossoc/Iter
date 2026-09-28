# iter serve: pages

## Shared frame (every page)
- Top bar: Home, Board (the current section is highlighted).
- Sidebar (hidden on board pages): organizations with their projects, then projects with no organization; the current item is highlighted.

## `/` Home
- Frame only; main pane says "Select an organization or a project."

## `/org/{id}` Organization (`?tab=tasks`)
- Info tab: description, settings (github, tmux, auto branch, branch template, default branch, github project), project list.
- Tasks tab: tasks of all its projects (task, project, status, issue); the proposal shows their count on the tab.
- Report tab (proposal only, `?tab=report&from=&to=`): the `iter org info` report — period presets and a date form, total, contents per project, then each project's tasks and sessions.
- Edit link.

## `/org/{id}/edit` Organization edit (POST saves)
- Name, description, settings, projects (comma-separated); error box; Save / Cancel.

## `/project/{id}` Project (`?tab=tasks`)
- Breadcrumb (organization).
- Info tab: description, base path, settings.
- Tasks tab: tasks (task, status, issue).
- Edit link.

## `/project/{id}/edit` Project edit (POST saves)
- Name, description, base path, organization select, settings; error box; Save / Cancel.

## `/task/{id}` Task
- Breadcrumb (organization / project).
- Description; status, GitHub issue, branch prefix, start, duration, tags (colored chips).
- Sessions table: start, end (or "ongoing"), time, message.
- Edit link.

## `/task/{id}/edit` Task edit (POST saves)
- Name, description, status, issue, branch prefix, start, duration, tags; error box; Save / Cancel.

## `/boards` Boards
- List of boards, or a hint to run `iter board new`.

## `/board/{id}` Board: agenda (`?date=`)
- Tabs: Agenda, Eisenhower matrix, Info, Edit.
- Unscheduled lists: important and not important.
- Day view with previous/next day and 24 hour slots; drag a card to schedule it (`board.js` posts to `/board/{id}/schedule`).

## `/board/{id}/matrix` Board: Eisenhower matrix
- "Not placed" list plus four quadrants; drag to place (posts to `/board/{id}/matrix`).

## `/board/{id}/info` Board: info
- Description and the board's projects.

## `/board/{id}/edit` Board edit (POST saves)
- Name, description, project checkboxes (noting a project's current board); error box; Save / Cancel.

## Assets
- `/style.css`, `/board.js`.
