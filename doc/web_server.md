# `iter serve`: the web server

A local topcoat server (`src/web/`), bound to loopback only, showing the same database the CLI uses.

## Requests and the database

Nothing is cached or copied. Every request opens the configured database file itself (WAL plus a busy timeout, so it coexists with a concurrent `iter` command), reads and writes through the same `Db` methods the CLI uses, and drops the connection. A `rusqlite::Connection` is not `Sync`, which is the other reason to open one per request.

Those SQLite calls block (up to 5 s of `busy_timeout` when the CLI holds the write lock). `blocking.rs` therefore runs each request inside `block_in_place`: the runtime moves the worker's other tasks to another thread first. One layer, no change to any handler. It needs the multi-threaded runtime that `serve` builds.

The layers, outermost first: `blocking::off_the_workers`, `guard::local_only`, `errors::error_pages`. Each page or route module exposes a `register` function that `mod.rs::router` folds into the builder.

## Route map

| Area | Routes | Module |
| --- | --- | --- |
| Home, settings | `/`, `/settings` | `home.rs`, `settings.rs` |
| Organization | `/org/{id}` (tabs), `/org/{id}/edit`, `/org/{id}/share` (POST), `/org/{id}/notes/{date}`, `/org/{id}/platform-activities` (JSON) | `org*.rs`, `daily_notes.rs`, `platform_pick.rs` |
| Project | `/project/{id}` (tabs), `/project/{id}/edit`, `/project/{id}/task/new`, `/project/{id}/notes/{date}` | `project*.rs`, `daily_notes.rs` |
| Task | `/task/{id}` (tabs), `/task/{id}/edit`, `POST /task/new`, `/task/{id}/{notes,sessions,runs,plan/{step},wiki/{action}}` (POST), `/task/{id}/prompt` (JSON) | `task*.rs` |
| Agents | `/agents`, `/agent/{id}/{n}`, `POST /task/{id}/runs/{n}[/run|/cancel]` | `agents.rs`, `task_runs.rs` |
| Boards | `/boards`, `/board/{id}` (agenda), `/board/{id}/matrix`, `/board/{id}/info`, `/board/{id}/edit`, `POST /board/{id}/{schedule,matrix}` | `board*.rs`, `agenda*.rs`, `matrix.rs`, `drop.rs` |
| Instructions | `/instructions`, `/instruction/{name}` | `instructions/` |
| Misc | `GET /search?q=` (JSON), `POST /folders/pick`, `/style.css`, `/board.js`, `/org.js`, `/platform.js`, `/favicon.ico` | `search_index.rs`, `folders.rs`, `assets.rs` |

The `{id}` of every route with one must be a number, else a 400 (`path_param!` in `mod.rs`). Static files are compiled in and served with an ETag and `Cache-Control: no-cache`: the browser revalidates and gets a bodiless 304.

## Tabs and pop-ups through query parameters

A page with sections (organization, project, task, note, agent) selects one with `?tab=`; an unknown or missing value is Info. `sections.rs` owns the parsing and builds the tab rows; `url.rs` is the only place URLs are concatenated; `ui/tabs.rs` only draws.

A pop-up is the same page with a parameter, so it works without JavaScript and closes by linking back:

- `?new=task`, `?new=project`, `?new=agent&instruction=NAME`, `?new=note`, `?new=session`, `?new=instruction`: the New pop-ups, opened by the first row of the matching list. Their POST routes redirect to the list on success and re-render the pop-up with the typed values and the error on refusal.
- `?share=1`: the Share pop-up over an organization's Report tab; the push preview is a pop-up too.
- `?run=N`, `?step=ID`: the agent or plan step open over the Agent tab.
- Board pages: `?date=` picks the agenda day, `?pick=<task>` turns the page into "choose where" (the pick mode, the no-drag way to move a task), `?moved=` marks the card just moved, `?side=off` folds the side column into its rail.
- `?q=` is a list's search (`task_query.rs` syntax); a Tasks link spells out the default search because a list with no `q` is unfiltered.

Drops (`board.js`) and pick forms post to the same routes (`drop.rs`). Pick forms also send a `form` marker or the day to land back on; the redirect URL is rebuilt from parsed values, never from request text.

## Edit forms

`edit.rs` is the shape every edit page shares. `GET` loads a row plus whatever else the form shows (`load`); `POST` applies the submitted form to the row (`EditForm::apply`), saves it (`save`, or `create_to` for a new one) and redirects to `EditForm::saved`. A submission that does not validate or save comes back as the form again, filled in, with the error and the field it is about marked invalid (`EditForm::field`, defaulting to the name field). The form types are in `forms/` (`OrgForm`, `ProjectForm`, `TaskForm`, `BoardForm`, the shared `SettingsForm`); `edit_page.rs` is the shared skeleton (frame, header, error box, Name, Description, Save / Cancel).

Notes on submissions: checkboxes are present (as `on`) only when ticked; repeated fields (the project checklists) are read as raw `(name, value)` pairs; textareas arrive with CRLF and are normalised to LF; a tag name that does not exist is reported before the write, so nothing is half-saved.

## The guard

`guard.rs` keeps the editor from being driven by anything but the user's own browser tab. The server listens on loopback, but a page on any website can still make the browser POST to `127.0.0.1` (CSRF) or point a hostile DNS name at it (rebinding). Two headers a web page cannot forge close both:

- `Host` must be a loopback name (`127.0.0.1` or `localhost`, any port), for every request.
- A state-changing request's `Sec-Fetch-Site`, when sent, must be `same-origin` or `none`, and its `Origin`, when sent, must equal `Host`. `Sec-Fetch-Site` backs `Origin` up: it names a cross-site request even if `Origin` was stripped.
- A request with neither header is not from a browser tab (a page's request always carries one) and is let through, for `curl`.

A refusal is a 403.

## Errors

`errors.rs` shows a document (title, skip link, landmarks, a way home) for the two refusals reachable by typing a URL: an id that is not a number (400) and one that names nothing (404). Topcoat's own answer is a bare text body. Only GET requests are rewritten, since the drop script's `fetch` wants the status. It is a pathless layer because a layer with a path never runs for a request that matched no route.

Action failures map to statuses by who can fix them: a refusal the user can act on (a project that is not a git repository, a name already taken, a missing harness) is a 400 carrying the message; a fault of the machine (database, disk, a program that would not start) is a 500 with the detail on the terminal, not echoed into a page.

## Other notes

- The folder button of a path field posts to `POST /folders/pick`, which opens the system chooser on the machine running the server (`osascript` on macOS, `zenity` elsewhere). A page cannot learn a picked folder's absolute path, but the server runs on the user's machine. Typed paths are cleaned of `.` and `..` segments first, and the folder reaches the chooser as an argument, never in the script text.
- Pages are built from shared components in `ui/`, which takes no models: `board_cards.rs`, `session_lines.rs`, `task_rows.rs` and `project_rows.rs` map models to what the components draw.
- Colors read from the database (the Urgent and Important tags) are used in `style` attributes, so only `#rrggbb` values are passed through.
- The footer is on every page, so what it reads is bounded (`FOOTER_LIMIT`); a footer that cannot be built is left out.
