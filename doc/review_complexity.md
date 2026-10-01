# Review: asymptotic optimality of the web UI

Lens: asymptotic cost of each handler, render and algorithm. Branch `feat/html-refactor`, worktree `html-refactor`.

## Scope

- Read: `src/web/**` (all page and handler modules, `forms/`, the `ui/` components that loop or build lists), `src/web/board.js`, `src/web/org.js` (only to confirm it is not hot).
- Read the functions they call in `src/db.rs`, `src/utils/report.rs`, `src/reporting.rs`, `src/config.rs`, `src/git.rs`.
- Not edited: no file under `src/`. The only file I wrote is this report. Throwaway scripts went to the scratchpad directory.

Symbols used below:

| Symbol | Meaning |
|---|---|
| T | tasks in the scope of the page (project, org or board) |
| U | unfinished tasks (status is not `done`) |
| P | projects in the whole database |
| O | organizations in the whole database |
| B | boards in the whole database |
| G | groups (orgs or boards) of one kind |
| S | sessions in scope |
| d | entries in a directory |
| L | string length |

## Method

What I did:

1. **Read the code and derived costs by hand.** I looked for nested scans, `contains` or `position` inside loops, per-row queries (N+1), repeated sorts and per-item allocation.
2. **Ran it.** I built the existing debug binary (`cargo build --features web` was already up to date) and ran `iter serve` against a throwaway SQLite database in the scratchpad (`XDG_CONFIG_HOME` pointed there, so the user's real DB was never touched).
   - **Task scaling.** I seeded 20 projects in one org and one board, with T from 1k to 64k. About 2/3 of tasks are unfinished, 1/5 are scheduled, 1/4 carry the Urgent tag and 1/6 the Important tag. There were 2T sessions. I timed `curl` against the pages (second request of each URL; debug build, so absolute numbers are inflated by a constant, but the growth is meaningful).
   - **Project scaling.** I seeded P from 250 to 16,000 projects with T=200, and timed the pages that should not depend on P.
   - **Validity check.** Each timing has an HTTP status and byte count next to it, so I know a 500 was not being timed. My first seeding attempts produced 500s (and curl failures); I discarded those numbers.
3. **Labels in this report.**
   - **[run]** = measured in the run above.
   - **[inferred]** = derived from code only.

What I did NOT check:

- Release-build numbers, concurrency or throughput under parallel requests.
- Browser-side cost: CSS, layout and paint of 20 MB pages, the `org.js` copy button.
- The `topcoat` view macro and router internals (how `view!` allocates, whether it streams or buffers).
- SQLite query plans (no `EXPLAIN QUERY PLAN`); claims about index use are inferred from the schema.
- CLI-only code paths, `sync.rs`, `github.rs`, `tmux.rs`.
- The `ui/` components I did not open (only grepped for loops and `contains`/`position`/`sort`). I found no nested iteration there.
- Memory use, apart from output sizes.

## Summary

- **No hidden O(n^2) found, in code or in the run.** Every page I timed grew linearly when T grew 4x: x4.2 to x4.7 per x4. There are no per-row queries; the group and board loaders use a fixed number of queries.
- **The real problems are bounds, not exponents.** Several pages do Theta(scope) work to produce O(1) or O(page) output.
- **Findings, in order of impact:**
  1. The footer makes every page O(P).
  2. Task filtering and reports read every row to show a few.
  3. Pages are unpaginated, up to 66 MB.
  4. Several smaller allocation constants.

Measured growth (warm, debug build, T tasks; [run]):

| Page | T=1k | T=4k | T=16k | T=64k | Output at 64k |
|---|---|---|---|---|---|
| `/org/1?tab=tasks` | 12.7 ms | 45 ms | 174 ms | 815 ms | 26.5 MB |
| `/org/1?tab=tasks&q=foo` | 5.0 ms | 13.5 ms | 47 ms | 211 ms | 5 KB |
| `/org/1?tab=tasks&q=tag:urgent` | 8.2 ms | 27 ms | 104 ms | 452 ms | 6.6 MB |
| `/org/1?tab=info` | 2.4 ms | 2.8 ms | 4.3 ms | 6.6 ms | 8 KB |
| `/project/1?tab=tasks` | 3.1 ms | 4.9 ms | 11 ms | 41 ms | 1.07 MB |
| `/board/1` (agenda) | 20 ms | 69 ms | 259 ms | 1.09 s | 20 MB |
| `/board/1/matrix` | 16 ms | 57 ms | 233 ms | 956 ms | 19 MB |
| `/org/1?tab=report` (30 days) | 60 ms | 232 ms | 971 ms | 4.4 s | 66 MB |
| `/org/1?tab=report` (1 day) | - | - | 62 ms | 242 ms | 4 KB |
| `/task/1` | 2.3 ms | 2.2 ms | 2.5 ms | 2.2 ms | 5 KB |

## Findings

### F1. The site footer makes every page Theta(P+O+B), and opens a second database connection

Severity: **Medium** (independent of the page; visible beyond a few thousand projects).

- `src/web/footer.rs:56-62` (`site_footer_for`) calls `open_db()` again and runs `footer::load` (`footer.rs:25-52`), which calls `grouped::<Organization>` (`load.rs:34`: `list::<Project>` + `list::<Organization>`) and `list::<Board>`. It renders a link for every project, org and board.
- [run] `/task/1` with T=200 tasks:

  | P | time | bytes |
  |---|---|---|
  | 250 | 3.1 ms | 14 KB |
  | 1,000 | 6.3 ms | 44 KB |
  | 4,000 | 18.9 ms | 170 KB |
  | 16,000 | 68.9 ms | 686 KB |

  Roughly 43 bytes of footer HTML per project, and the rest of the page does not change. The same growth appears on `/project/1`, `/task/1/edit` and `/project/1/edit`. The cost is O(P+O+B) work and bytes per request, whatever the page is.
- The footer opens its own `Db` (`create_dir_all`, three pragmas, `user_version` read) on top of the page's connection. Every request therefore pays two opens (`Db::open`, `db.rs:228`). A refused project create pays four: `add`, `create_to`, `list_page` and the footer (`project_new.rs:131,209-226,240`).
- **Optimal bound:** O(page). The footer is navigation, not data.
- **Fix:** (a) cap the footer (first k projects, or only org and board names, with a "more" link to `/`); or (b) make it static, such as a link to `/`; or (c) at minimum reuse the page's `Db` instead of opening another, and cache the rendered footer string behind a version counter (SQLite `PRAGMA data_version`) so the unchanged case costs O(1).

### F2. Report and task filters read Theta(T) rows to display few

Severity: **Medium** (grows with history; done tasks accumulate forever).

- **Org report.** `org_report::load` -> `utils/report.rs:122-154` (`project_reports`) calls `db.tasks_for_projects(&ids)` (`db.rs:961`), which loads every task of every project in the org, including `description` and other long strings. It then discards the ones with no session in the period (`report.rs:69-71`). The sessions query is range-bounded and indexed, so the output depends only on S in range, but the task read is O(T).
  - [run] a one-day report (4 KB output): 62 ms at T=16k, 242 ms at T=64k. That is linear in T, not in output.
  - **Optimal:** O(S_range + tasks that have a session in range).
  - **Fix:** read only those tasks, with one query joining `sessions` to `tasks` (and `projects`) over the range.
- **Query box.**
  - `task_query.rs:164-181` (`keep`), called from `org.rs:89` and `project.rs:66`. The tab first loads every task of the scope (`db.tasks_in`, `db.tasks_for_project`), then filters in Rust. [run] `q=foo` (5 KB output) took 47 ms at 16k tasks and 211 ms at 64k, against 4 ms and 7 ms for the Info tab at the same sizes. An all-`is:done` no-match query behaves the same (196 ms at 64k).
  - For `tag:` terms, `keep` collects the id of every task into one JSON array and loads all tag names for all of them (`db.tag_names_for_tasks`, `db.rs:971`), instead of letting SQL answer `EXISTS (tag)`.
  - The default `is:open` filter could also be a SQL `WHERE`.
  - **Optimal:** substring search over names is Theta(T*L) without a full-text index. The constants and the allocation can be cut: select only `id, name, status, project_id, github_issue` (not `description`, `branch_prefix`...), push status and tag terms into SQL, and `LIMIT` the result.
  - **Fix:** build the `WHERE` clause from the parsed terms. The terms are already typed (`Kind::Status`, `Kind::Tag`...). Matching in Rust can stay for the free-text terms.
- **Matcher allocation.** `task_query.rs:149-160` (`matches`): `task.name.to_lowercase()` and `project.to_lowercase()` allocate once per term per task; `tags.iter().any(|t| t.to_lowercase() == *name)` allocates per tag per term. That is O(T*k*L) allocations where O(T*L) lowercasing and no extra allocation would do.
  - **Fix:** lowercase `name` once per task, or use an ASCII-case-insensitive contains, and `eq_ignore_ascii_case` for tags.
- **Needless query.** `org.rs:89` calls `filter_named` even on the Info and Report tabs, where `tasks` is empty. With a `tag:` term in `?q=` that still runs a `tag_names_for_tasks([])` query (the guard `needs_tags` does not check emptiness). One wasted query, not a scaling problem. [inferred]

### F3. No pagination or limits: output bytes and time are unbounded in T and S

Severity: **Medium** (worth a decision; today's linear cost is the symptom).

- [run] see the growth table above: at T=64k the org Tasks tab is 26.5 MB, the agenda 20 MB, the matrix 19 MB and the 30-day report 66 MB.
- The report page also puts the text of every task into one `data-copy` attribute (`org_report.rs:128-138` `status_list`, used in `report_view`), so the task list is sent twice.
- These are linear, not quadratic, and a real board is likely to have hundreds of rows. The lens question is what the optimal bound is: O(page size) per request.
- **Fix:** add `LIMIT`/`OFFSET` (or keyset on id) to the Tasks tab, with a "showing N of M" line (the count already exists, `count_tasks_in`). For the report, cap sessions per task or paginate by project. For boards, cap the Backlog list.

### F4. Board loading reads every unfinished card even when the page shows a count or little

Severity: **Low-Medium**.

- `board_page.rs` (`load_board`) loads all cards via `db.cards_unfinished` (`db.rs:417`) and sorts them. [run] `/board/1/matrix?side=off` (a 6 KB page, since the Backlog is folded to a count) still took 215 ms at 64k tasks versus 7 ms for an Info page: O(U) to print a number.
- `db.rs:899` (`tasks_of`) looks up the project name per task with `names.get(...).cloned()`, one `String` allocation per task, and `with_priorities` (`db.rs:423`) builds a second `String` per task (`task_ref`). A shared `Rc<str>` or a join in SQL would avoid one of the two.
- `status <> 'done'` (`db.rs:21`) has no index: SQLite visits all tasks of the board's projects, including `done` ones that accumulate. The `UNIQUE(project_id, name)` index narrows to the projects but not to unfinished. A partial index `ON tasks(project_id) WHERE status <> 'done'` would make the read O(U) instead of O(T). [inferred, no EXPLAIN]
- Double sort: `load.rs:64` (`unfinished`) sorts all cards by `(urgent, label)` (O(U log U * L)). `agenda_day.rs:54` (`split`) then stable-sorts each bucket again by `rank` (`agenda_day.rs:79`), and `quadrants.rs:92` stable-sorts `waiting` by `status != Wip`. The second sorts are a stable partition by a 2-bit key, which is O(U) with buckets. The extra cost is small and it is not quadratic. [inferred]
- **Fix:** if the page only needs a count, use `SELECT COUNT(*)` (as the tab badge already does); sort once with a composite key; add the partial index; fold the project name in the SQL (`JOIN`) rather than a `HashMap` + clone.

### F5. Folder picker allocates inside the sort comparator and has no cap

Severity: **Low** (bounded by one directory, but `d` is user-chosen and unbounded).

- `folders.rs:79`: `names.sort_by_key(|name| name.to_lowercase())` allocates a new `String` for every key evaluation, about 2 d log d allocations. `sort_by_cached_key` makes it d allocations.
- `folders.rs:71-80` (`subfolders`): `entry.path().is_dir()` is one `stat` per entry (needed to follow symlinks) and builds a `PathBuf` per entry. Total O(d) syscalls, which is optimal. The page then emits d links (`folders.rs:100-107`: `name.clone()` plus `dir.join(name)` plus a URL) with no cap. A directory with 50k entries makes a 50k-row page.
- `start_folder` (`folders.rs:55`) walks `ancestors()` doing `is_dir` per level: O(depth). Fine.
- **Fix:** `sort_by_cached_key`; optionally cap the listing with a "N more" line.

### F6. Repeated sort passes in session totals

Severity: **Low**.

- `utils/report.rs:60-84` and `:122-154`: `merged_total_minutes` (`reporting.rs:181`) copies and re-sorts the sessions of a task, then of its project (the union), then of the whole org (`org_report.rs:75`). That is three O(S log S) passes over data the database already returned ordered by `start`. The per-task sort is on an already-sorted run (Rust's stable sort is O(S) on sorted input), so only the concatenated unions pay.
- Not quadratic. A k-way merge of the per-task runs would give O(S log k).
- `total_minutes` also calls `config()` (RwLock read) each time; negligible.

### F7. Per-card URL building: constant-factor allocations

Severity: **Info**.

- `agenda_cards.rs` (`Cards::view`) builds `links(board, day)` per card, which formats the day URL, then `pick()` runs `page_url`, then `with_side`. That is about six `String`s per card.
- `board_cards.rs` (`CardCtx::view`) adds `task_url`. `task_card` formats `card-{id}` and `card-{id}-title`.
- Linear, a few hundred bytes per card. The day-URL prefix is the same for every card of a page; build it once per request. [inferred]

### F8. Per-request connection set-up

Severity: **Info**.

- `Db::open` (`db.rs:228`) runs on every request: `create_dir_all`, `PRAGMA busy_timeout`, `foreign_keys`, `journal_mode=WAL`, `user_version`. This is O(1), and the `migrate` doc comment says it is on purpose.
- But `prepare_cached` statements live on the connection, so across requests the cache never hits. Every query is re-prepared. Constant factor only; a small pool or a long-lived connection would remove it.
- [run] baseline `/boards` with an almost empty database was about 1.8 ms. Most of that is two opens (page plus footer) in a debug build.

### Checked and found optimal

- `load::grouped` (`load.rs:34`): two queries and one pass with a `HashMap`, O(P+G).
- `db.priorities_of` (`db.rs:465`): one join query per board instead of per card, with `idx_task_tags_tag`.
- `db.tasks_in` / `tasks_of`, `sessions_for_projects_in_range`, `tag_names_for_tasks`, `ids_by_name` (chunked), `assign_projects`: set-based with `json_each`, constant query count.
- `project_options::build` (`project_options.rs:62`): `HashMap` for group names and `HashSet` for the ticked ids, O(P+G+k). `Pairs::ids` sorts and dedups, O(k log k).
- `edit::load_among` (`edit.rs:96-107`): one `list` plus `position`, O(G), done once per request.
- `agenda_day::split` and `quadrants::split`: a single pass, fixed 24 or 4 buckets (`hours.iter().all(Vec::is_empty)` is O(24)).
- `pick::picked_in` (`pick.rs:30`): a linear `find` (twice per page, for `pick` and `moved`). O(U), dominated by the load.
- `project_rows::matching` (`project_rows.rs:43`): O(P*w*L) with one `format!` per project; fine for P in the thousands.
- `board.js`: one `querySelectorAll` at load, then event delegation with `closest`. No per-event scans, no listeners per card. The drop reloads the page (`location.replace`), which costs the full O(U) render of F4 for a one-card move. That is a design choice, not a bug.
- Static assets: ETag hash computed once (`OnceLock`), `no-cache` revalidation gives a 304 with an empty body (`assets.rs`).
- `url::page_url` is O(L) per call (`format!("%{b:02X}")` per escaped byte allocates a temporary; negligible).

## Complexity table

Query counts are SQL statements per request, not counting the footer's (F1). The footer adds O(P+O+B) to every row [run].

| Function | file:line | n | Derived cost | Optimal | Verdict |
|---|---|---|---|---|---|
| `site_footer_for` / `footer::load` | footer.rs:56, 25 | P, O, B | 3 queries + a second connection; O(P+O+B) work and bytes per page | O(1) | F1 [run] |
| `load::grouped` | load.rs:34 | P, G | 2 queries, O(P+G) | O(P+G) | optimal |
| `home` / `boards_page` | home.rs:26, boards.rs:27 | P | O(P+O) / O(P+B) | O(P) | optimal (inherent: they list every project) |
| `org::load` Tasks tab | org.rs:72-101 | T_org | 3 queries; reads all T rows, then filter in Rust: Theta(T) | O(T) scan only for free text, O(page) otherwise | F2, F3 [run] |
| `org::load` Info tab | org.rs:72-101 | P_org | 1 `COUNT` + O(P_org) | O(P_org) | fine [run: 7 ms at 64k] |
| `project::load` Tasks tab | project.rs:58-72 | T_proj | 1 query O(T_proj) + filter | as above | F2, F3 [run] |
| `TaskQuery::matches` | task_query.rs:149 | k terms | O(k*L) allocations per task | O(L) | F2 |
| `TaskQuery::keep` | task_query.rs:164 | T | 1 tag query over all T ids, O(T + tag rows) | SQL `EXISTS` | F2 |
| `org_report::load` | org_report.rs:65 | T_org, S_range | 2 queries + 3 sort passes: O(T_org + S log S) | O(S_range) | F2, F6 [run] |
| `utils::project_reports` | utils/report.rs:122 | T, S | O(T + S log S) | O(S) | F2, F6 |
| `merged_total_minutes` | reporting.rs:181 | S | copy + sort + scan: O(S log S) | O(S) on pre-sorted | F6 |
| `report_view` / `status_list` | org_report.rs:198, 128 | T_r, S_r | O(S_r) + O(T_r) duplicated in `data-copy` | O(S_r) | F3 |
| `task::load` | task.rs:48-59 | S_task | 4 queries; `sessions_for_task` uses the `(task_id,start)` index | O(S_task) | optimal (page cost then dominated by F1 [run]) |
| `load_board` | board_page.rs | U | 4 queries, O(U log U * L) sort | O(U), or O(1) for a count | F4 [run] |
| `db.cards_unfinished` | db.rs:417 | U | 3 queries + O(U) clones | O(U) | F4 (no partial index) |
| `agenda_day::split` | agenda_day.rs:54 | U | O(U + 24) + O(U log U) bucket sorts | O(U) | fine (second sort redundant) |
| `agenda::show` render | agenda.rs | U | O(U) with ~6 allocations per card | O(U) | F7 |
| `quadrants::split` | quadrants.rs:92 | U | O(U) + stable partition sort | O(U) | fine |
| `matrix_page` render | matrix.rs | U | O(U) | O(U) | fine |
| `pick::picked_in` | pick.rs:30 | U | O(U), twice | O(U) | fine |
| `drop_on` / `place` / `schedule` | drop.rs:35 | - | 2 point lookups + 1 transaction, O(1) | O(1) | optimal |
| `folders::browse` / `subfolders` | folders.rs:83, 71 | d | O(d) stats, sort allocates 2 d log d | O(d log d), d allocations | F5 |
| `project_rows::matching` | project_rows.rs:43 | P | O(P*w*L) | same | fine |
| edit pages: `group_options` | project_options.rs:40 | P, G | 2 queries, O(P+G) | O(P+G) | optimal (inherent: a checklist of all projects) |
| `edit::load_among` | edit.rs:96 | G | 1 `list`, O(G) | O(G) | optimal |
| `edit::submit` / `settle` | edit.rs:136 | - | `get`, one write transaction, O(1) | O(1) | optimal |
| `task_new::save` refused path | task_new.rs:207-268 | T, P | re-runs the full list load (`load_org`/`load_project`): O(T) | O(T) | acceptable (error path) |
| `project_new::add`, `list_page` | project_new.rs:209, 238 | P | 4 connections on a refused POST | 1 connection | F1/F8 |
| `Db::open` | db.rs:228 | - | O(1), once per request (twice with the footer) | O(1) | F8 |
| `board.js` | board.js | cards | O(cards) at load, O(1) per event | O(cards) | optimal |

## Confidence

- **High** that nothing in the audited scope is quadratic: all cost functions are sums and products of single scans, and the measured growth ratios confirm it [run].
- **High** on F1 (clean, strongly linear measurement, one cause) and on the F2 report/filter reads (a 4 KB output costs time linear in T).
- **Medium** on the size of the F4 and F5 gains. The index claim in F4 is inferred from the schema and I did not run `EXPLAIN QUERY PLAN`.
- **Medium** on F7/F8 constants (debug build, no profiler). They are constant-factor only.
- One side observation from running, unrelated to complexity: `str_to_dt` (`db.rs:56`) panics on a malformed stored timestamp, so one bad row turns every page that reads it into a 500. My first seeding attempt hit this.

## What I could not assess

- Release-build performance, parallel request behaviour, and whether `block_in_place` per request (`blocking.rs`) limits throughput.
- How `topcoat`'s `view!` builds output (whether it buffers or streams). If it buffers, the 20-66 MB responses in F3 also cost peak memory O(output).
- Browser rendering of very large pages; this may dominate before the server does.
- SQLite query plans and page-cache effects on a cold, large database file. All my numbers are warm reads.
- Behaviour beyond P=16,000 projects, and path names with pathological lengths.
