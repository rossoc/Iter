# Board matrix redesign (`/board/{id}/matrix`)

> Superseded (final cleanup, `doc/web_cleanup.md`): the compare switch and `?design=old` were removed; the pages render the current design only. References below to the old design, the switch or `proposal` (now `screen`) describe how it was built, not how it is.


Scope: the board's Eisenhower matrix (page 10 of `doc/web.md`). The info and edit pages (11, 12) stay on the old shell until their turn. This page is built almost entirely from the agenda's pieces (`doc/web_board_agenda.md`): task card, drop zone, unplaced list, pick mode, sticky banner, jump link, failed-drop alert, `board.js` (unchanged).

## Plan

### Page
1. `ui::frame` with `current: BOARDS`, title "Eisenhower matrix - {board} - iter", no sidebar (`Nav::load` only for `?design=old`).
2. Top: `board_header(board, Matrix)` (crumbs `Boards / Eisenhower matrix`, board name, Edit, tabs Agenda | **Eisenhower matrix** | Info).
3. Body in `info_columns` as on the agenda: the matrix on the left (the hero), the **Backlog** list on the right (`side_lists`, sticky, drop target `left`). Under 900px one column: the matrix first, in reading order 1-4, then Backlog; a `jump_link` "Skip to the backlog" (visible when narrow, focus-only when wide) reaches the list.
4. **Four quadrants**, numbered, in reading order (row by row):

   | # | Name | Rule (shown) | Flags a drop sets | Drop target |
   |---|---|---|---|---|
   | 1 | Do first | Important, urgent | urgent + important | `both` |
   | 2 | Plan | Important, not urgent | important | `important` |
   | 3 | Delegate | Not important, urgent | urgent | `urgent` |
   | 4 | Eliminate | Not important, not urgent | neither | `neither` |

   Each quadrant is a section named by its heading (numeral, name, count; the rule is its description), holding a drop zone with cards or a quiet line ("Nothing to do first.", "Nothing to plan.", "Nothing to delegate.", "Nothing to eliminate."). Two columns (1 2 / 3 4) when there is room, one column when the matrix is narrower than about 500px (`flex-wrap`, no breakpoint: the matrix sits beside a list at some widths, so the viewport is the wrong measure).
5. **Color is meaning only, never the only cue.** A quadrant has a 4px top edge in the board's own priority colors, the same as the cards' left edge (`edge-*`): Do first = Urgent and Important side by side, Plan = Important, Delegate = Urgent, Eliminate = neutral. The numeral, the name and the rule are text, so the edge is decoration. Text is on `--bg`, never on those colors (13:1 and up). No new colors, no fills at rest; a dragged card over a quadrant gets the drop zone's dashed accent outline and tint.
6. **Cards** are `task_card`: in Backlog they keep their chips (the flags are what the card brings along); in a quadrant the chips are left off (the quadrant says it, as the Important chip on the agenda's Important list). Link "Place" (Backlog) / "Move" (placed).
7. Colors from board settings: `board_root` (`--urgent`, `--important`, only `#rrggbb`).
8. `?design=old` shows the previous matrix (`compared`, GET only) with the switch, keeping `?pick=`.
9. Empty board (no unfinished task): `empty_state` instead of the matrix, as on the agenda.

### Placing without a mouse (accessibility)
10. Drag and drop is progressive enhancement: `board.js` (unchanged) posts to `/board/{id}/matrix` and reloads; a failed drop says so in the `role=alert` note and does not reload.
11. **Pick mode, no script**: a card's **Place** / **Move** link (hidden text: the task) opens `?pick={task}#pick`. `pick_bar`, in a `sticky_head`: "Moving {task} (now Do first). Choose a quadrant." (a task not placed yet: "Placing {task}. Choose a quadrant."; the verbs and the `moving` accent are the agenda's) with **Cancel** (back to the card) and, when the task is placed, **Unplace** (back to Backlog). Every quadrant gets a **Place here** (**Move here**) button, first in it (before the cards, so it is the first tab stop after the banner) (hidden text " at Do first"), a `post_button` form with `task_id` and `target`; none in the quadrant the task is already in. The picked card is marked "Being moved" (words, dashed border).
12. **Endpoint**: `POST /board/{id}/matrix` behaves as before (`task_id`, `target`; answers `true` as JSON; unknown quadrant 400; a task of another board 404; same-origin guard `guard::local_only`). New and optional: a `form` field, sent by the pick forms only (a page without script has no `board.js` to reload); when present the route answers `303` (`back_or`, as the agenda) to `/board/{id}/matrix#card-{task}`, a URL built from the ids, never from the request. Focus lands on the moved card (the fragment; `board.js` does it after a drop's reload).
13. `prefers-reduced-motion`: nothing new moves (the card lift is already behind `no-preference`); forced colors: the top edge (a background) becomes a 4px `CanvasText` border, `.over` keeps its outline.

### Speed
14. Queries: the board (one lookup), the two tag colors (one query), the unfinished cards (`Db::cards_unfinished`, three queries however many tasks), sorted once (`load::unfinished`). One pass (`split`) puts each card in Backlog or in its quadrant by an index computed from its two flags (`quadrant_index`, O(1), no scan of the table): O(n). The picked card is found once. No sidebar for the proposal.

### Components
Reused: `frame`, `board_header`, `board_root`, `info_columns`, `sticky_head`, `pick_bar`, `pick_here`, `post_button`, `jump_link`, `side_lists`, `unplaced_list` (with `target: "left"`), `drop_zone`, `task_card`, `count_badge`, `empty_line`, `empty_state`, `compared`, `page_url`, board URL helpers, `board.js`.
New, one feature each (`src/web/ui/`, rules next to them under `.v2`, ui takes no models):
- `quadrant`: one numbered, named, counted section holding a drop zone; the top edge takes the card edge's classes (`Edge::modifier`).
- `matrix_grid`: the two-column wrap of the four quadrants.
Web-level: `web/quadrants.rs` (the four quadrants as data, `split` into buckets, `place`; tested), `web/matrix.rs` (route, loading, pick words, the POST route).
Changed: `Edge::modifier` (the class part shared by cards and quadrants); `pick_fields` takes the name and value of its "where to land" field (the agenda's `date`, the matrix's `form`).

### What stays old for pages 11-12
`board.rs` keeps info, edit, save, `board_tabs`, the old `zone` / `card`, the old agenda and (only for `?design=old`) the old matrix view, and the `.board`, `.card`, `.zone`, `.matrix`, `.q-*` rules of `style.css` they use. Triggers for deleting: `old_matrix` and the `.matrix` / `.q-*` rules go with `?design=old` in the final cleanup (with `old_agenda`); `board_tabs` with the info page (11); the old `card` / `zone` and `.board` / `.card` / `.zone` rules once both old boards are gone. Nothing of the old matrix is used by the proposal: its bucketing and `QUADRANTS` were replaced by `quadrants.rs`, which the old view now also uses.

### Feature changes
- Added: the header with tabs and Edit, numbered quadrants with a name, the rule text and a count, priority colors as an edge (meaning), the pick mode (keyboard / no-JS placing and unplacing), the failed-drop alert, the jump link, the title with the section, the compare switch.
- Changed: quadrants are outlined sections (hairline, colored top edge) and not 25% color fills; cards show chips, time and status as on the agenda; the "Urgent / Not urgent" axis labels are replaced by each quadrant's own rule text (it reads the same in one column).
- Removed: nothing (the old matrix stays under `?design=old`).

## Checked against the plan

| # | Plan | Status | Where |
|---|---|---|---|
| 1 | Frame, Board highlighted, title, no sidebar | done; title "Eisenhower matrix - Daily - iter"; `Nav::load` only for `?design=old` | `matrix.rs` |
| 2 | `board_header(Matrix)` | done | `board_header.rs` |
| 3 | `info_columns`, Backlog on the right, jump link when narrow | done (jump link visible at 320px) | `matrix.rs` |
| 4 | Four numbered quadrants, 2 columns / 1 | done; 2 columns at 1280 and 900, 1 at 320; order 1-4 | `ui/quadrant.*`, `ui/matrix_grid.*`, `quadrants.rs` |
| 5 | Meaning-only color, text cues | done; forced colors: 4px border top (checked) | `ui/quadrant.css` |
| 6 | Cards: chips only in Backlog; Place / Move | done | `matrix.rs` `Cards` |
| 7 | Colors from board settings | done (`board_root`) | `matrix.rs` |
| 8 | `?design=old` | done; still shows the old matrix | `board::old_matrix` |
| 9 | Empty board | done (`empty_state`) | `matrix.rs` |
| 10 | Drag and drop kept, failed-drop alert | done; drag 55 to Delegate and back checked in a browser; a stubbed 500 shows the alert and does not reload | `board.js` unchanged |
| 11 | Pick mode | done; Enter on Place lands on `#pick`, Place here at Plan, Unplace, Cancel checked; focus ends on `card-55` | `pick.rs`, `ui/pick_bar.*` |
| 12 | Endpoint | done; form marker gives 303 to `/board/1/matrix#card-55`, without it `true`, bad target 400, other board 404 | `matrix.rs` `place_task` |
| 13 | Reduced motion, forced colors | done (transition `0s`, border 4px) | `quadrant.css` |
| 14 | Queries, one pass | done; `split` O(n), index from the flags (tests) | `quadrants.rs` |

Verified (throwaway DB copy via `XDG_CONFIG_HOME`, own ports): matrix default, pick (unplaced and placed task), bad `pick`, empty board, at 1280, 900 and 320px, light and dark: axe 0 violations, no horizontal scroll. Agenda, info and edit still answer (their old axe findings predate this). `cargo fmt --check`, `cargo clippy --features web`, `cargo test --features web` (250), `cargo test --no-default-features` (174).

Open issues: dragging a card to a quadrant changes its flags (as before), so a task dragged back to Backlog keeps the new flags; after "Place here" the page returns to the card, not to the top; HTML drag and drop is unreliable on touch browsers, the pick mode is the fallback; the old matrix (`?design=old`) has legacy axe findings.

## Review

Five reviewers (accessibility, complexity, composability, DRY, design). Each finding verified first.

| Finding | Status |
|---|---|
| A1 a placement is never announced | fixed: both paths load `?moved=<task>#card-<task>` (form: 303; drag: `board.js` `location.replace`, no `sessionStorage`); the page words it from the task's stored state ("Moved p/x to Plan", "... to Not placed"; agenda: "... to Tue 29 Sep, 10:00", "... to Not scheduled") and `board_root(note)` puts it in the `role=status` `.board-notes`; the script rewrites it after load (a live region reads a change) and takes `moved` out of the URL. Same on the agenda. A bad or foreign `moved` says nothing. Differs from the suggested `sessionStorage`: one path for form and drag, and the wording is the server's |
| A2 banner name | fixed: `aria-labelledby` the message ("Moving p/x (now Plan). Choose a quadrant.") |
| A3 44px targets | fixed: `.button.small` gets `min-height:44px` under `pointer:coarse` in `button.css` (the hour grid's copy removed); checked as a rule, the emulation has no coarse pointer |
| A4 first tab stop | fixed: `pick_here` first in the quadrant and, same gap, first in the hour slot on the agenda; tab order: Cancel, Skip link, Place here (first quadrant) |
| A5 duplicate landmark | fixed: `unplaced_list(alone: true)` has no `aria-labelledby` on the matrix (the aside is the labelled one); the agenda's two lists stay regions |
| A6 list semantics for cards | skipped (optional): the cards are `<article>`s and the heading has the count ("3 tasks"); `ul`/`li` would change `drop_zone`, the hour rows and `board.js` for the agenda too. Revisit in the final cleanup |
| A7 title in pick mode | fixed: "Placing p/x - Eisenhower matrix - Daily - iter"; agenda "Scheduling / Moving p/x - Tue 29 Sep - Agenda - ..." |
| C1 one transaction | fixed: `Db::update_placed(id, task, Option<Priority>)` (test); no chips built for placed cards (`Chips::None`; the agenda's Important list uses `Chips::Except`); `picked_in` also gives `placed` and `priority`, so `Matrix::quadrant_of` (a second scan) is gone |
| P1 legacy dependency | fixed: `web/drop.rs` (`Drop`, `drop_on`, `back_or`); `schedule`, its route and `schedule_back` moved to `agenda.rs`; `board.rs` is info, edit and the old views only. `#[serde(flatten)]` was tried and fails on urlencoded numbers (`invalid type: string "7", expected i64`), so `Drop` is one form with optional `date` and `form` |
| P2 generic loader | fixed: `board_page.rs` (`load_board`, `BoardPage<D>`, `BoardLoad<D>`), `CardCtx` with `view` / `view_chips`, `ui/board_empty`, the script tag folded into `board_root`; no shared page skeleton |
| P3 `pick.rs` split | fixed: generic in `pick.rs` (`Picked`, `picked_in`, `pick_fields`, `pick_link`, `card_link`, `moved_link`, `moved_note`, `pick_title`), agenda words in `agenda_pick.rs`, matrix words in `matrix.rs`; `PICK_ANCHOR` in `pick_bar` |
| P4 `Edge` | fixed: `Edge::of(Priority)` (test for the four), `Quadrant.edge` removed, `quadrants.rs` has no `ui` import |
| P5 `flex`, class name | fixed: the basis is in `matrix_grid.css` (`.quad-grid>.quad`); class renamed `quad-grid` |
| D1 one mapping | fixed: `Edge::of`; `quadrant_index` and `flags_at` side by side, tested against each other |
| D2 `edge.css` | fixed: `.edge`, `.edge-*`, `--edge-w`, `--q1`, `--q2`; `Edge::modifier` returns the base class too |
| D3 old code | kept on purpose: `old_matrix`, `old_agenda`, `board_tabs`, `Card::class` / `slot`, `zone`, `card` and the `.matrix`, `.q-*`, `.board` rules of `style.css` go with `?design=old` (final cleanup); `board_tabs` with the info page (11) |
| S1 verbs | fixed: a placed task is "Moving" / "Move" / "Move here" with the `moving` accent, else "Placing" / "Place" / "Place here"; `doc/web_components.md` said every verb was an accent, now only `moving` |
| S2 names | fixed: "Plan" (design 04 already said Plan), "Nothing to eliminate.", parallel empty lines, rules read "Important, ..." / "Not important, ..."; design 03 and 04 record the look (1-4 numerals, top edge instead of fills, 4px card edge) |
| S3 inner list | fixed: `--r-md`, 4px padding, dashed only when empty or `.over`; in forced colors the border is `Canvas` (a transparent one is forced visible; found in the check) |
| S4 tokens | fixed: `--num-w` (24px, the label indent), `--head-gap`, `--pad-x`, `--quad-gap`, `--quad-min`, shared `--edge-w`; `min-height` 88px; heading 600 / -.015em |
| S5 stale docs | fixed: `doc/web_components.md` (frame signature, "shared by the agenda and the matrix", the `Drop` form, `board.js` contract), `doc/web.md` |

Verified again (throwaway DB copy, own port): axe 0 violations and no horizontal scroll on matrix default, pick (placed, unplaced, bad id), empty board, agenda default, pick (scheduled, unscheduled) and empty board, at 1280, 900 and 320px, light and dark. Mouse drag between quadrants and back, and on the agenda: status message, focus on the card. Simulated 500: alert, no reload. Keyboard pick on both pages: banner name, tab order, verbs, messages. Reduced motion (transition 0s) and forced colors checked; `?design=old`, info, edit, boards, home, project and task pages answer 200. `cargo fmt --check`, `cargo clippy --features web`, `cargo test --features web` (257), `cargo test --no-default-features` (175).

## Update: same card as the agenda

Quadrant and "Backlog" cards call `CardCtx::view` like the agenda's: no flag chips (the edge and a hidden priority phrase carry the priority), start time only when scheduled. The pick slot stays the small button (the big dashed slot is the agenda's).
