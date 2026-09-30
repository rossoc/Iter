# Boards page redesign (`/boards`)

> Superseded (final cleanup, `doc/web_cleanup.md`): the compare switch and `?design=old` were removed; the pages render the current design only. References below to the old design, the switch or `proposal` (now `screen`) describe how it was built, not how it is.


Scope: the list of boards (page 8 of `doc/web.md`). It is also the entry to the board section, so it introduces the pieces pages 9-12 (agenda, matrix, info, edit) share: board URLs, the board header with its section tabs, and the board loader. Those pages are not redone here. The page is Home's sibling (`doc/web_homepage.md`): eyebrow, heading, a grid of cards.

## Plan

### Page
1. `ui::frame` with `current: "/boards"`, so the top bar's Board link is highlighted (it already links here). Document title "Boards - iter". No sidebar (boards are their own section), so `Nav::load` runs only for `?design=old`.
2. Header: `page_header` with the eyebrow "Board" (the section name, as in the top bar; "Plan your day" was dropped), `h1` "Boards", then a `lede`: "Select a board to plan its tasks."
3. One **card per board**, in the Home card grid: the board name as a linked `h2` (arrow icon, `aria-hidden`), the first line of its description (two lines at most, as the notes on Home cards), then its projects as compact rows (name link, path in mono). A board with no projects reads "No projects yet." (as on Home). Projects with no board are not listed: they are not on any board.
4. Empty state (`empty_state`, the kanban icon): "No boards yet. Create one with `iter board new`."
5. `?design=old` shows the old list (`compared`, GET only), with the switch, as the other redesigned pages.

### Shared groundwork for pages 9-12
6. **Board URLs** (`ui/url.rs`): `BOARDS` (`/boards`, also used by the frame's nav), `board_url(id)` (the agenda, `/board/{id}`), `board_matrix_url`, `board_info_url`, `board_edit_url`. The routes' `POST` URLs stay where they are (`board.js`).
7. **Board header** (`web/board_header.rs`, one feature: the top of every board page): breadcrumb kicker `Boards / {section}` (Boards linked, the section as plain text because the tabs carry `aria-current`), `h1` the board name, the Edit button (`board_edit_url`) on the right, then the section tabs **Agenda | Eisenhower matrix | Info** (`ui::tabs`, `aria-label` "Board sections"). Edit is the header button, not a tab (as on Org and Project). `BoardSection { Agenda, Matrix, Info }` gives the tab labels and the page-title part. Not used until page 9, so it carries `#[expect(dead_code)]`, which page 9 removes (the compiler then reminds whoever forgets).
8. **Board loaders** (`web/load.rs`): `grouped::<G>(db)` (groups with their projects plus the loose ones; boards page, `Nav::load`, and the board info page's project list), `board_of(db, id)` (one lookup by key, 404 when missing), next to `org_of` / `project_of`. Pages 9-12 call it.
9. **`group_card`** (`ui/group_card.rs`, one feature: an overview card, a linked or plain title, a note, its project rows or an empty line) and **`card_grid`**, moved out of `home.rs` and `home.css` so Home and Boards share them. `first_line` (first line of markdown, was `summary`) moves to `ui/prose.rs`. `home.css` is deleted (its rules become `group_card.css`).

### Speed
10. Two queries, however many boards: `list::<Board>` and `list::<Project>`, grouped in one pass by `load::grouped::<Board>` (shared with `Nav::load` for organizations). No per-board query.

### Accessibility
11. Landmarks: header, `nav` "Main" (Board `aria-current`), `main`; the grid is a `section` of `article`s each headed by an `h2`; project rows are a `ul`. The h2 link's name is the board name; the arrow is `aria-hidden`. Title slot, skip link, focus ring, contrast and reduced motion come from the shared sheet; the card lift and the arrow's nudge are inside `@media (prefers-reduced-motion:no-preference)` (`ui/group_card.css`), so under reduced motion the card only changes shadow and border.

### Feature changes
- Added: eyebrow and lede, description first line and the board's projects on each card, the empty-state hint styled as on Home, the compare switch, the shared board header, URLs and loader.
- Changed: the plain list becomes a card grid; the empty text loses its backticks (`iter board new` is `<code>`).
- Removed: nothing.

### Existing board code kept for pages 9-12
`board.rs` keeps the agenda, matrix, info, edit and save pages, `board_tabs`, `zone`/`card`, `colors_style`, the drop routes and their tests, all on the old shell until each page is redone; only `boards_page` leaves it (now `web/boards.rs`). `board.js` and its `POST` routes are untouched.

## Checked against the plan

| # | Plan | Status | Where |
|---|---|---|---|
| 1 | Frame, Board highlighted, title "Boards - iter", no sidebar in the proposal | done; `aria-current` on Board, `Nav::load` only for `?design=old` | `boards.rs`, `ui/frame.rs` |
| 2 | Eyebrow, `h1`, lede | done | `boards.rs` |
| 3 | One card per board: name, first description line, projects | done; long name wraps, board with no projects says "No projects yet." | `ui/group_card.rs`, `boards.rs` |
| 4 | Empty state with `iter board new` | done (empty database checked) | `boards.rs` |
| 5 | `?design=old` | done; the switch keeps `/boards` | `boards.rs` |
| 6 | Board URLs | done (`BOARDS`, `board_url`, `board_matrix_url`, `board_info_url`, `board_edit_url`; unit test) | `ui/url.rs` |
| 7 | Board header with tabs | written and unit-tested, not rendered yet: `#[cfg_attr(not(test), expect(dead_code))]` on the module, its URL helpers and the loader; page 9 removes them (the compiler flags any left) | `board_header.rs` |
| 8 | Board loader | `load::board_of` (test: missing board is an error) | `load.rs` |
| 9 | `group_card` / `card_grid` shared with Home; `summary` in `prose.rs`; `home.css` deleted | done; Home renders the same (axe 0, both themes) | `ui/group_card.*`, `home.rs` |
| 10 | Two queries | done: `list::<Board>` and `list::<Project>`, grouped by `board_id` (test) | `load::grouped` |
| 11 | Accessibility | done; see below | |

Not in the plan (found while testing):
- The `<code>` chip in the empty state (`--muted` on `--bg-3`) was 4.33:1 in light mode, so axe flagged it (Home's empty state has the same chip). `.empty code` is now `--fg`.

Verified (throwaway database copy through `XDG_CONFIG_HOME`, own ports; one variant with three boards, one with an empty database): `/boards` at 1280px and 320px, light and dark: axe 0 violations, no horizontal scroll, title "Boards - iter", one `h1` and one `h2` per board. Keyboard: skip link, brand, Board, then each card's title and project links in order, all with a 2px outline (forced colors: the outline is kept). `?design=old` renders the old list (axe 0). `/`, `/org/1`, `/board/1`, `/board/1/matrix`, `/board/1/info`, `/board/1/edit`, `/project/1`, `/task/1` still answer 200; Home and Org axe 0. `cargo fmt --check`, `cargo clippy --features web` (no new warnings), `cargo test --features web` (223 passed), `cargo test --no-default-features` (174 passed).

## Review

| Finding | Result |
|---|---|
| A1 card lift / arrow nudge under reduced motion | fixed: the transforms live in `@media (prefers-reduced-motion:no-preference)`; checked with emulation (reduce: `none`, no-preference: `translateY(-2px)`) |
| A2 duplicate link names across cards | accepted: a fix needs hidden path text in the shared `project_list` (also used by Org) or per-row ids; the row's path is beside the link and each card is an `article` headed by its board |
| C1 one branch on `old` | fixed: `Loaded::Old(Nav, boards)` / `Loaded::New(grouped)` chosen once; `old_list` is its own component; no `Option<Nav>`, no unused queries (a `Db` is not `Sync`, so components cannot take it) |
| P1 `board_heading` + tabs, `tabs_for` | fixed: `board_heading(board, here)` (page 12 uses it alone), `board_header` = heading + tabs; `tabs_for(current, entries)` + `Tab::link` used by `section_tabs` and `board_tabs`; `BoardSection` and `ui::Section` stay separate (path vs query based) |
| P2 shared loader | fixed with D1: `load::grouped` |
| P3 `summary` -> `first_line`; `NO_PROJECTS` | fixed (`NO_PROJECTS` in `ui/group_card.rs`, used by both cards) |
| D1 grouping written twice | fixed: `load::grouped::<G: Table + ProjectGroup>` returns `Grouped { groups, loose }` (`Nav::load` keeps `loose`); test moved to `load.rs` and extended |
| D2 hard-coded `/boards` | fixed: `BOARDS` in `layout.rs` |
| D3 hand-built `/board` paths, dead_code expects, old-shell CSS | kept on purpose; grep checklist below |
| S1 Home "No projects yet." size | fixed: `.v2 .group .empty-line{font-size:.875rem}` (14px, as before) |
| S2 eyebrow | fixed: eyebrow "Board", `h1` "Boards", the idea stays in the lede |
| S3 `web_homepage.md` stale | fixed: superseded notes added |

Checklist for pages 9-12 (run when each page lands, remove what no longer matches):
- `grep -n '"/board\|format!("/board' src/web/board.rs` : hand-built paths, replace with `board_url` & co.
- `grep -rn 'expect(' src/web/mod.rs src/web/load.rs src/web/ui/url.rs` : `dead_code` expects for `board_header`, `board_of`, board URL helpers.
- `grep -n 'fn board_tabs' src/web/board.rs` : the old tabs, gone with page 9-11.
- old-shell CSS for `.crumbs`, `.tabs .edit`, `.boards` in `src/web/style.css`, and `Nav::load` calls in `board.rs`, go when the last board page is redone.

Verified again after the fixes: `/boards` (three boards plus long name / no projects, and an empty database) and Home at 1280px and 320px, light and dark: axe 0 violations, no horizontal scroll; `?design=old` renders; `/org/1`, `/project/1`, `/task/1`, `/board/1`, `/board/1/matrix`, `/board/1/info`, `/board/1/edit` answer 200; tab order is skip link, brand, Board, then each card's title and project links. `cargo fmt --check`, `cargo clippy --features web`, `cargo test --features web` (224), `cargo test --no-default-features` (174) pass.
