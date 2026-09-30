# Board agenda redesign (`/board/{id}?date=`)

> Superseded (final cleanup, `doc/web_cleanup.md`): the compare switch and `?design=old` were removed; the pages render the current design only. References below to the old design, the switch or `proposal` (now `screen`) describe how it was built, not how it is.


Scope: the board's day agenda (page 9 of `doc/web.md`). The matrix, info and edit pages (10-12) stay on the old shell until their turn; this page builds the pieces they will reuse (task card, drop zone, `board.js`). It follows Boards (`doc/web_boards.md`) and uses the board header that page prepared.

## Plan

### Page
1. `ui::frame` with `current: BOARDS` (Board highlighted), title "{day} - Agenda - {board} - iter" (the day, so history and tabs tell days apart), no sidebar (`Nav::load` only for `?design=old`).
2. Top: `board_header(board, Agenda)`: crumbs `Boards / Agenda`, `h1` the board name, Edit button, tabs **Agenda | Eisenhower matrix | Info**. This removes the `dead_code` expects listed in `doc/web_boards.md`.
3. Body in `info_columns` (the day on the left, the lists on the right, one column under 900px, the day first): **time first**, the day is the hero, the unscheduled lists are a side panel.
4. **Day** (left): heading with the date (`h2`, a `<time>`), then `day_nav`: previous day, **Today**, next day, all plain links (`?date=yyyy-mm-dd`, no JS). Then the 24 hour slots as an ordered list (`hour_grid` / `hour_slot`): the hour in mono, a hairline between hours, the cards scheduled at that hour (bucketed by start hour, as before). Today only: a **now line** (a `--line-w` (2px) accent line and a dot, placed by the server at `minutes / 60` of the current hour's row, no JS), the row carries `aria-current="time"` and the line has hidden text "Now, 09:25".
5. **Lists** (right, `side_lists`): "Important, not scheduled" and "Not important, not scheduled" as `labelled_section`s with a count, each a drop zone holding cards, or `empty_line` ("Nothing important waiting." / "Nothing else waiting."). Sticky beside the day, static when narrow or short (as the panel), scrolls inside itself when taller than the screen.
6. **Task card** (`task_card`, new): white-ish surface, hairline border, a 4px left edge colored by priority (Urgent's color, Important's color, or the two split for both; the colors are the board's Urgent and Important tags). The edge is decoration: the flags are also **chips with the words "Urgent" / "Important"** (`tag_chips`, ink computed for 4.5:1, as tags on the task page). Content: the title (`project/task`, link to the task), mono time (`09:00-10:00` in a slot; the duration `1h 30m` when unscheduled), the chips, the `status` pill only for work in progress, and a small **Schedule / Move** link.
7. **Drop zone** (`drop_zone`, new): `data-target` is what `board.js` posts; on hover with a dragged card it gets a dashed accent outline and a tint. Used by every hour, the two lists, and later the matrix's quadrants.
8. **Colors from board settings**: `--urgent` and `--important` on the board root (`board_root`, new: `data-post` plus the two custom properties), set only when the stored color is a valid `#rrggbb` (the database accepts any text, and it goes into a `style` attribute). Text is never on those colors: cards are `--bg-2` with `--fg` text (13:1 and up), chips get black or white ink. Color carries meaning only (priority, now line, drop target), never alone.
9. `?design=old` shows the previous agenda (`compared`, GET only) with the switch. The switch keeps `?date=`.

### Scheduling without a mouse (accessibility)
10. Drag and drop is progressive enhancement: `board.js` (no framework) keeps HTML5 drag and drop for mouse and touch (long press where the browser supports it) and posts to the same endpoint, then reloads.
11. **The keyboard / no-JS path is a pick mode, plain links and forms, no script**: a card's **Schedule** (or **Move**) link opens `?date=D&pick={task}#pick`. The page then shows a banner (`#pick`, focus target: the fragment moves the keyboard starting point there) "Scheduling {task}. Choose an hour." with **Cancel** (and **Unschedule** when the task is scheduled), and every hour row gets a **Schedule here** button (a tiny `<form method="post">` to `/board/{id}/schedule`; hidden `task_id`, `target` = `yyyy-mm-dd hh:00`). Chosen because it needs neither JS nor per-card selects (25 options times N cards would bloat the page): the extra DOM (24 forms) exists only while picking. The link text carries the task name (hidden text) so a screen reader hears "Schedule work/api".
12. **Same endpoint, same behavior.** `/board/{id}/schedule` still answers `true` (JSON) to `board.js`. New and optional: a `date` field (from the pick forms only); when present and valid, the route answers `303` to `/board/{id}?date=...` so a form post lands back on the agenda. The route builds the URL from the parsed date, so there is no open redirect. Same-origin guard unchanged (`guard::local_only`).
13. `prefers-reduced-motion`: the card's lift and the zone tint change are inside `no-preference`; nothing slides. The shared sheet already zeroes transitions.

### Speed
14. Queries (after the review): the board (one lookup), the two tag colors (one `IN` query, `Db::priority_colors`), and its unfinished cards (`Db::cards_unfinished`: `status <> 'done'` is in the SQL of both the tasks and the priorities, so finished work is never read; three queries however many tasks), no per-card query. One Rust sort (`load::unfinished`), and the CLI's `Db::cards` keeps its own order. One pass (`split`) puts each card in the important list, the other list or its hour bucket (`[Vec<Card>; 24]`), dropping done and other-day cards; O(n), no nested scans. The picked card is found with one `find` before the split. The old design's sidebar is loaded only for `?design=old`.

### Components
Reused: `frame`, `board_header` (and so `page_header`, `breadcrumb`, `edit_button`, `tabs`), `info_columns`, `labelled_section`, `empty_line`, `status`, `tag_chips`, `compared`, `page_url`, board URL helpers, `button`.
New, one feature each (`src/web/ui/`, rules next to them, all under `.v2`; ui takes no models, `web/board_cards.rs` maps `Card`s):
- `task_card`: one card (title link, time, chips, status, action link).
- `drop_zone`: a place cards can be dropped (`data-target`).
- `board_root`: the board's wrapper (`data-post`, priority colors).
- `day_nav`: the date heading with previous / Today / next.
- `hour_grid` / `hour_slot`: the list of hours and one row (label, zone, now line).
- `side_lists`: the aside holding the unscheduled lists.
- `pick_bar`: the banner of the pick mode.
Web-level (not `ui/`): `board_cards.rs` (`Card` to `CardView`, colors, time text), `agenda_day.rs` (`split`, tested), `agenda.rs` (route, load, assembly).

### What stays old for pages 10-12
`board.rs` keeps the matrix, info, edit and save pages, `board_tabs`, `zone`/`card`, `place`, the drop routes and their tests, and the old agenda view (only for `?design=old`). `board.js` serves both: it now finds cards, zones and the board by their `data-` attributes, which the old markup already has. Triggers for deleting: `board_tabs`, the old `card`/`zone` and the `.board`, `.card`, `.zone` rules in `style.css` go with page 10 (matrix) and 11 (info); the old agenda view goes with `?design=old` in the final cleanup.

### Feature changes
- Added: the header with tabs and Edit, Today link, now line, priority chips (words, not only color), duration on unscheduled cards, status on work-in-progress cards, the pick mode (keyboard / no-JS scheduling and unscheduling), title with the day, the compare switch, the Today link lands on the current hour (`#now`).
- Changed: card color moves from a 35% fill to a left edge; lists sit beside the day, sticky; the previous/next links are labeled ("Previous day", "Next day", with the date).
- Removed: nothing (the old agenda stays under `?design=old`).

## Checked against the plan

| # | Plan | Status | Where |
|---|---|---|---|
| 1 | Frame, Board highlighted, title with day, no sidebar | done; title "Tue 29 Sep - Agenda - Daily - iter"; `Nav::load` only for `?design=old` | `agenda.rs` |
| 2 | `board_header` (crumbs, name, Edit, tabs); dead_code expects removed | done; expects gone from `mod.rs`, `load.rs`, `ui/url.rs` (grep clean) | `board_header.rs` |
| 3 | `info_columns`, day left, lists right, stacks under 900px | done (day first when stacked) | `agenda.rs` |
| 4 | Day: heading, day_nav, 24 hours, now line | done; line placed by the server, `aria-current="time"`, hidden "Now, 07:56" | `ui/day_nav.*`, `ui/hour_grid.*` |
| 5 | Lists as `labelled_section` with counts and empty lines, sticky | done | `agenda.rs` `waiting`, `ui/side_lists.*` |
| 6 | Task card: edge, chips with words, time/duration, status for wip, action | done; the edge classes are `edge-*` (`.u` and `.i` are the link line and the icon: a first try collapsed the cards) | `ui/task_card.*`, `board_cards.rs` |
| 7 | Drop zone | done; used by hours and both lists | `ui/drop_zone.*` |
| 8 | Colors from board settings, validated | done (test: only `#rrggbb` reaches the style); text never on those colors | `board_cards.rs` `Colors`, `ui/board_root.rs` |
| 9 | `?design=old` | done; the switch keeps `?date=` | `board::old_agenda` |
| 10 | Drag and drop kept | done; mouse drag to 15:00 and back to the list checked in a browser | `board.js` |
| 11 | Pick mode | done; keyboard: Enter on Schedule, focus lands on `#pick`, Enter on "Schedule here at 11:00"; Unschedule and Cancel checked | `ui/pick_bar.*`, `post_button` |
| 12 | Same endpoint; optional `date` gives a 303 back to the agenda | done; without `date` still `true`; date parsed, URL rebuilt | `board.rs` `schedule_task` |
| 13 | Reduced motion | done (transitions `0s` under emulation) | `task_card.css` |
| 14 | Queries and one pass | done; `split` is O(n) into 24 buckets (tests) | `agenda_day.rs` |

Changed from the plan: the scroll-to-now script was dropped. It moved the keyboard's starting point, so the first Tab landed mid-page and skipped the header and tabs. The Today link (`#now`) does it on request instead. The day heading is sticky under the top bar so the day and its arrows stay in reach.

Verified (throwaway database copy via `XDG_CONFIG_HOME`, own port): `/board/1` today, tomorrow, yesterday, pick mode (scheduled and unscheduled task), and an empty board, at 1280px and 320px, light and dark: axe 0 violations, no horizontal scroll, titles per day. Tab order: skip link, brand, Board, Boards, Edit, Agenda, Eisenhower matrix, Info, then the cards. Bad `date` / `pick` fall back to today / no pick. `?design=old` renders (its old axe findings predate this). Matrix, info, edit, `/boards`, `/` answer 200 and are unchanged. `cargo fmt --check`, `cargo clippy --features web`, `cargo test --features web` (234), `cargo test --no-default-features` (174).

Open issues: (a task scheduled on another day is now in the folded "Scheduled on other days" list, see Review); the now line is drawn at load and does not move until reload; HTML drag and drop is unreliable on touch browsers, the pick mode is the fallback; after "Schedule here" the page returns to the top, not to the hour.

Triggers for old code: `board_tabs`, old `card`/`zone`, `.board/.card/.zone` rules in `style.css` go with pages 10-11; `Nav::load` in `board.rs` and `old_agenda` go with `?design=old`.

## Review

Five reviewers (accessibility, complexity, composability, DRY, design). Old-design duplicates and their exact triggers: `board.rs` `Card::class` / `Card::slot` and the old `card` / `zone` components, and the `.board`, `.card`, `.zone`, `.hour`, `.matrix` rules of `style.css`, go when the matrix (page 10) is converted and `?design=old` is dropped for the board pages; `board_tabs` when the info page (11) is converted; `old_agenda` (and its use of `neighbours` / `board_day_url`) with the `?design=old` branch in the final cleanup. `Card::slot` (`09:00-10:00`) is not merged with `time_text` (en dash, duration, other-day ends): the old page would change, so it waits for that cleanup (`slot_text` skipped for that reason).

| Finding | Result |
|---|---|
| A1 pick keeps `pick` on day links, Today, `here`; banner shows task and day | fixed: `pick::day_url`, `today_url`, `here` carries `pick`; banner "Moving X (now Mon 28 Sep, 13:00)"; a task of another day is moved with Next / Move here (checked) |
| A2 focus after actions | fixed: cards are `#card-<id>` (`tabindex=-1`); the schedule redirect and Cancel end on it; `board.js` focuses the moved card after its reload (`sessionStorage`). No separate status line: the focused card is read out |
| A3 sticky vs focus | fixed: tokens `--bar-h`, `--sticky-h`, `--stick-top` on `html`, `scroll-padding-top` from them; `.sticky-head` (heading + banner) is static under 900px or 600px high like `.side`/`.panel` |
| A4 picked card | fixed: dashed accent border, `--bg-3` fill and the visible words "Being moved" (one text for eye and screen reader, so no separate sr text); still `aria-current`. Card is an `<article>` labelled by its title |
| A5 failed drop, JS-only affordances | fixed: `role=alert` note in `.board-notes`, no reload on failure (checked with a 500); `draggable` and the grab cursor / dashed targets come from `board.js` (`.can-drag`) |
| A6 Today shape, over outline, hours labelled, count text, target size | fixed: Today underlined; `.over` outline in forced colors; `hour_grid(label_id)`; "(N tasks)" via `count_badge`; `.tcard-act` 28px (44px coarse) |
| A7 jump link | fixed: `jump_link` "Skip to tasks not scheduled" (focus-only when wide, always visible when narrow) |
| C1 unfinished in SQL, one sort, one tag query | fixed: `Db::cards_unfinished`, `priority_colors` (one `IN`), `load::unfinished` sorts once; `Db::cards` (CLI) unchanged; tests |
| C2 other-day tasks | fixed: third list "Scheduled on other days" (folded `<details>`, date on each card, Move link; open when the picked card is in it), filled in the same pass (`Day::elsewhere`) |
| P1 `unplaced_list`, `side_lists(label)`, drop-zone variant | fixed (`drop_zone(class)`, `.drop-list`) |
| P2 `pick.rs`, `pick_bar` words, `pick_here`, wording | fixed; "Moving" / "Move here" for a scheduled task, "Scheduling" / "Schedule here" otherwise |
| P3 redirect helper, `date` only on schedule | fixed: `back_or`, `schedule_back` (validated, rebuilt), `ScheduleForm` vs `Drop`; tests |
| P4 `unfinished` in `load.rs`, `draggable` default, `board.js` contract | fixed; contract in the `board.js` header and `web_components.md` |
| D1 one hex validator | fixed: `models::is_hex_color` used by `board_cards` and `ink.rs` |
| D2 `board_day_url` | fixed (+ test), used by pick URLs, the redirect and `old_agenda` |
| D3 `move_fields`, `board_of`, imports, `neighbours`, `slot_text` | `pick_fields` is the one helper (a second `move_fields` would repeat it); `board_of` in matrix and info; imports; `neighbours`; `slot_text` skipped (see above) |
| S1 sticky banner with accent | fixed: banner in `sticky_head` under the bar, 4px left border, `data-mode` accent (`--ok` for Moving); Cancel and Unschedule stay in reach (static when narrow) |
| S2 tokens | fixed: `--line-w`, `--edge-w` (4px, as documented), `--dot`; drop lists `--r-lg`; the doc said 1px, now 2px; picked card differs from the focus ring |
| S3 stacked layout | fixed: jump link, gap 28px under 900px (`.info`, so other pages with a panel share it) |
| S4 Important chip, empty hours, touch, transitions, shadow clipping | fixed: no Important chip in its list; empty hours 44px with the same dashed target as the lists; `(pointer:coarse)` sizes; `--fast` on `.tcard-act`; `.side` padding with equal negative margin |
| S5 empty board | fixed: `empty_state` (icon and hint) replaces the lists, the hour grid stays |

Verified (throwaway DB copy, own port): axe 0 violations on default, other date, empty day, pick (unscheduled, scheduled, other-day task, other day) and empty board, 1280 and 320, light and dark; no horizontal scroll; a contrast failure on the picked card (light) was found and fixed (`--fg` meta). `cargo fmt --check`, `cargo clippy --features web`, `cargo test --features web`, `cargo test --no-default-features`.
