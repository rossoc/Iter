# Board info page redesign (`/board/{id}/info`)

> Superseded (final cleanup, `doc/web_cleanup.md`): the compare switch and `?design=old` were removed; the pages render the current design only. References below to the old design, the switch or `proposal` (now `screen`) describe how it was built, not how it is.


Scope: the Info tab of a board (page 11 of `doc/web.md`): its description and its projects. The edit form (page 12) is left for later and keeps working. The page is the sibling of the Org and Project Info tabs (`doc/web_org.md`, `doc/web_project.md`) and reuses the board header of the agenda and matrix (`doc/web_board_agenda.md`, `doc/web_board_matrix.md`).

What the old page shows, and so all the new one shows: the board's name, its tabs, the description, and the list of its projects (names only, "No projects on this board." when empty). A board has no other fields: no settings, no colors of its own (the urgent/important colors are tags' colors, used by the cards on the agenda and matrix, not a board setting). So there is no settings panel, and no `info_columns` (that layout is a body plus an aside; with no aside it would leave an empty column).

## Plan

1. Frame: `ui::frame` with `current: BOARDS`, title "Info - {board name} - iter" (the same shape as "Eisenhower matrix - {name}"). No sidebar: `Nav::load` only runs for `?design=old`.
2. Header: `board_header(board, BoardSection::Info)` as on the agenda and matrix: breadcrumb `Boards / Info`, the name as `h1`, the Edit button, tabs Agenda / Eisenhower matrix / Info with Info `aria-current`.
3. Body, one readable column (the width of Org's left column), in this order:
   - the description (`prose::description`, line breaks kept, nothing when empty);
   - `projects_section(projects, empty)` (`web/project_rows.rs`, shared with Org info): the "Projects" label with the count as a bare-number badge, holding `project_list` (name link, path in mono, `~` for the home directory) or `empty_line("No projects on this board.")` (Org passes "No projects.").
   The body is wrapped in `info_body` (`ui/columns.rs`): `max-width: var(--info-body)`, the width the body has beside the aside (`columns.css` derives it from the same `--info-aside` / `--info-gap` as `.info`).
4. Empty description shows nothing, as on Org and Project. A board with no description and no projects shows the header and the Projects label with its empty line.
5. `?design=old` shows the old page (`compared`, GET only) with the switch, as on the other GET pages.

### Speed
6. Two queries: the board (`board_of`, by key) and its projects (`projects_in::<Board>`, one query). No sidebar and no per-project query. The old design loads the sidebar on top, only for `?design=old`.

### Accessibility
7. Landmarks and headings as on the other board pages: header, `nav` "Board sections", `main`; `h1` the board name, the section is an `h2` named by its heading (`aria-labelledby`); the project list is a `ul`; the badge is the bare number (the label names the noun). Rows wrap: a long name breaks anywhere and the path drops below it, no horizontal scroll at 320px; only the compact cards on Home and Boards cut a path with an ellipsis (its `title` has the whole).

### Components
- Reused: `frame`, `board_header` (`page_header`, `edit_button`, `tabs`, `breadcrumb`), `prose::description`, `projects_section` (new, `web/project_rows.rs`), `info_body` (new, `ui/columns.rs`), `empty_line`, `compared`.
- New code: `web/board_info.rs` (the route, the loader and the page, nothing else). `web/pages.rs`-style split as on the other pages: the route stays thin.

### Old code
8. `board.rs` loses the old info route (`old_info` stays in `board.rs`, used only under `?design=old`); its tabs helper is now `old_board_tabs`, used by `old_info`, `old_agenda` and `old_matrix`; the edit page keeps `edit_page`, `save_page`, `BoardForm`. `old_info`, `old_agenda`, `old_matrix` go with the `?design=old` removal at the end; the edit page goes when page 12 lands.

### Feature changes
- Added: the project count on the section label; the projects' base paths; the compare switch.
- Changed: same look as the other Info tabs (frame, header, tabs, prose, project rows); the projects are rows, not a bulleted list; the sidebar is gone.
- Removed: nothing.

## Checked against the plan

| # | Plan | Status | Where |
|---|---|---|---|
| 1 | Frame, title "Info - {name} - iter", no sidebar | done; `Nav::load` only for `?design=old` | `board_info.rs` |
| 2 | `board_header` with Info current | done | `board_header.rs` |
| 3 | Description, Projects section with count and `project_list` or empty line | done; board with description and no projects, with neither, and with 8 projects checked | `board_info.rs` |
| 4 | Empty description shows nothing | done | `ui::prose::description` |
| 5 | `?design=old` with the switch | done (old page unchanged) | `board::old_info` |
| 6 | Two queries | done: `board_of`, `projects_in` | `board_info.rs` `load` |
| 7 | Accessibility | done | see below |
| 8 | Old code | old route body kept as `old_info` (used by `?design=old`); `board_tabs` kept for the old agenda and matrix | `board.rs` |

Checked in a browser (throwaway copy of the database via `XDG_CONFIG_HOME`): boards with 8 projects and a long unbroken word in the description, with a description and no projects, and with neither; 1280px and 320px, light and dark, normal and reduced motion (axe 0 violations on all 12 combinations per board; no horizontal scroll), forced colors active (screenshots load, no scroll). Keyboard order: skip link, brand, Board, Boards crumb, Edit, Agenda, Matrix, Info, project links. Titles "Info - Daily - iter"; unknown board is a 404. Agenda, matrix, edit, boards and `?design=old` still render.

Not in the plan: nothing found. Old design's document title stays "iter" (predates this work).

Verified: `cargo fmt --check`, clippy (0 warnings), 257 tests with `web` and 175 without.

## Review

| Finding | Status |
|---|---|
| A1 long names force horizontal scroll | fixed: rows `flex-wrap`, name `overflow-wrap:anywhere`; checked Board info (70-char unbroken name), Org, Home, Boards at 320px: no scroll |
| A2 path clipped; `direction:rtl` + `<bdi>` | fixed: paths wrap on info pages; ellipsis and `title` only for `compact`; `rtl` and `<bdi>` dropped (paths read left to right) |
| A3 "Projects (1 projects)" | fixed: Projects badge is the bare number; `count_badge` says "1 task" for one (tested) |
| A4 tabs wrap | fixed: `flex-wrap`, column gap 24px; Org, Project, Board tabs checked at 320px (one row there; the task page has no tabs). Wrap itself only reasoned, not seen |
| A5 forced colors badge | fixed: 1px transparent border, `CanvasText` under forced colors (seen) |
| S1 readable column | fixed: `info_body`, `--info-body` token beside `.info` |
| S2 Projects parity | fixed: `projects_section(projects, empty)` on Board and Org info, badge on both |
| P1 `Nav::for_design` | skipped: the four Old/New loaders carry different data, one helper would not remove the enums; all disappear with `?design=old` |
| D1 page titles | fixed: `BoardSection::X.label()` in agenda and matrix |
| D2 stale docs and comments | fixed: this file, `board.rs` header, `old_board_tabs` |
| D3 old code | kept; triggers recorded under Old code |

Re-checked: axe 0 violations on Board info (3 boards), agenda, matrix, Org, Home, Boards, Project, Task at 1280 and 320, light and dark (reduced motion in dark), no horizontal scroll; keyboard order; forced colors. `?design=old` shows the old page (axe heading-order and a 320px scroll there predate this work).
