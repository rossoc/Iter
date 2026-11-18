# Review 2026-11-02 — performance

Lens: algorithmic complexity and measured performance of the web routes, the common CLI commands and the agent/ingest paths that build the wiki graph. Release binary `target/release/Iter` of HEAD `5d96f44`, macOS (Darwin 25.5), stores seeded with `doc/review_tools/seed.sh` at scale 1 / 4 / 16 (2–8 orgs, 5/20/80 projects, 30/120/480 tasks, 200/800/3200 sessions), then filled with `doc/review_tools/performance/fsload.sh` (365 notes per org, 60 per project, 3 agent runs per task). Notation: n = tasks, s = sessions, p = projects, o = organizations, N = note files, W = wiki pages, R = run records, L = wiki levels (≤ 3).

## Summary

The database side is in good shape: every query in `src/db/*.rs` hits an index (`EXPLAIN QUERY PLAN` on the scale-16 store), handlers run a fixed number of statements, and every page that reads only the database answers in 1.2–2.6 ms at scale 16, growing ×1.1–1.7 for ×16 data. The old complexity findings about the footer (now bounded to 8 rows, one window query) and the report N+1 (now two batched queries) are fixed. The CLI is flat at ~30 ms at every scale, of which ~24 ms is the measuring harness and ~6 ms process start-up; no CLI command's time depends on the store size.

The costs that remain are in the filesystem and process-spawning paths, and they are large, not asymptotically wrong:

1. **`/agents` takes 1.7 s at scale 16 (103 → 418 → 1698 ms, ×16.4) because `RunRecord::shown_status` spawns `kill -0` and `ps` (5.3 ms a pair) for every record marked `running`; the task Agent tab pays the same 9 ms per running record.** Setting those records to `done` drops `/agents` to 43 ms. One `ps -p a,b,c` (or `libc::kill(pid, 0)`, no spawn) per request is the fix. (High)
2. **`/search` reads and parses every note file of every organization and project on every request: 19 → 80 → 150 ms for 971 → 3884 → 6776 notes, per keystroke.** Cache the entries per `serve` process behind the notes folders' mtimes and `PRAGMA data_version`. (High)
3. **`/instructions`, `/instruction/{name}` and `/agents` walk every task's folder** (`usage()`): one `SELECT organization` per task, one `read_dir` and R YAML parses; 3.6 → 12 → 42 ms. (Medium)
4. **The wiki graph is built where only the levels are needed**: `ingest::auto` builds the full oxigraph store (90 ms at 2000 pages, measured) before reading `settings.yaml` to learn the mode is `manual` (the default), on `session new`, `task done` and every `solve begin`; `/task/{id}?tab=wiki` builds it plus three `git` spawns per render (36 ms empty, 122 ms at 2000 pages); `?tab=agent&new=agent` builds it for the prompt (94 ms at 2000 pages). The build itself is linear and parallel (8 → 90 ms for 200 → 2000 pages). (Medium)
5. **Every page opens the database two or three times** (page, footer, and a third in `/task/{id}?tab=agent` and the three edit pages); an open with its PRAGMAs costs ~0.6 ms against ~4 µs per query, so the opens are 30–50 % of a 1.5 ms page. `migrate()` after the first open is one `PRAGMA user_version` read, not a cost. (Medium, small absolute)

Nothing super-linear was found anywhere; the derived bounds and the growth ratios agree.

## Scope

- Every GET route of the brief's route map plus `/task/{id}?tab=agent&new=agent`, `/org|project/{id}?tab=notes`; five `/search` queries. POST routes were read, not timed (they share the loaders).
- CLI: `project`, `task`, `project info`, `org info` (day, 30 days, worklog), `session start/stop` (tmux-less path), `notes list`, `wiki find/show/check/pending/export`.
- Agent/ingest: `ingest::auto`, `Target::new`, `scope::wiki`, `Wiki::build/fill/entail`, `retrieve::find`, `check::check`, `RunRecord::shown_status`, `TaskDir::runs`.
- Database: every statement in `src/db/{mod,groups,tasks,sessions,reporting,migrate}.rs`.
- Not in scope: build times, binary size (build lens), tmux-driven paths, concurrency under load.

## Method

Environment per the brief (`XDG_CONFIG_HOME` under the scratch dir, `ITER_PLATFORM_KEY=dummy`, `TMUX` unset). Nothing under `src/`, `Cargo.*` or `~/.config/iter` was touched.

- Seed: `doc/review_tools/seed.sh $SCRATCH/perf{1,4,16} {1,4,16} $BIN`; servers `$BIN serve -p 400{1,2,3} --no-open`; a fourth scale-1 store `perfw` on 4004 for the wiki experiment.
- Routes: `doc/review_tools/performance/routes.sh` (curl `-w '%{http_code} %{size_download} %{time_total}'`, median of 11 on the bare seed, median of 7 after `fsload.sh`; each pass run twice, second pass kept). Raw numbers: `doc/review_tools/performance/results/routes_{seedonly,loaded}_x{1,4,16}.tsv`.
- Filesystem load: `fsload.sh <store> <server-pid>` (notes, runs; run 3 of each task `running` with the server's pid so `alive()` has a live process).
- Wiki: `wikigen.sh` (0 / 200 / 2000 pages into the org-1 wiki, notes folder under git) + `wiki_bench.sh` (CLI median of 7, pages median of 7). Results: `results/wiki.txt`.
- CLI: `cli.sh` (hyperfine is not installed; Python-timed loop, median of 7, baseline row `true` included to show the harness cost ≈ 23 ms). Results: `results/cli.txt`.
- Query plans: `/usr/bin/sqlite3 $SCRATCH/perf16/iter/iter.db "EXPLAIN QUERY PLAN ..."` for 18 statements copied from `src/db`.
- Open cost: Python `sqlite3` doing the same four PRAGMAs/reads on a copy of the scale-16 file, 500 iterations (an approximation of `Db::open`; rusqlite's constant differs slightly).
- Process spawn cost: 100 `kill -0` + `ps -o stat= -p` pairs from Python.
- Files read: `src/web/{mod,footer,load,layout,blocking,search_index,search,task,task_agent,task_runs,task_wiki,task_query,org,org_report,org_share,project,home,agents,daily_notes,board_page,edit,project_edit,org_edit,task_edit,project_new}.rs`, `src/web/instructions/{mod,list}.rs`, `src/db/*.rs`, `src/wiki/{graph,scope,ingest,check,retrieve,ontology.ttl}.rs`, `src/wiki/ingest_job/{mod,review,lock}.rs`, `src/agent/{run,solve,task_prompt,taskdir,instruction}.rs`, `src/commands/{session,task,project,organization,wiki,notes,ingest,misc}.rs`, `src/utils/{report,resolve}.rs`, `src/reporting/{worklog,merge}.rs`, `src/notes.rs`, `src/push.rs`, `src/process.rs`.
- Older reports consulted: `doc/review_complexity.md`, `doc/web_final_review.md` §2, `doc/web_cleanup.md` batch A.
- NOT checked: concurrency (one client at a time), browser render cost of the large pages (`/board/1` is 74 KB, `/agents` 346 KB at scale 16), POST handlers' timing, tmux-driven `session new`, the ingest agent itself.

## Findings

### Table

| ID | Sev | Where | Evidence | Effort | One-line fix |
|---|---|---|---|---|---|
| P1 | High | `src/agent/run.rs:169-204`, `src/web/task_runs.rs:189`, `src/web/agents.rs:63` | run | S | Probe all pids with one `ps -o pid=,stat= -p a,b,c` per request (or `libc::kill(pid,0)`, no spawn) and memoise per render |
| P2 | High | `src/web/search_index.rs:156-160,125`, `src/notes.rs:115-128` | run | M | Build the search entries once per `serve` process; invalidate on `PRAGMA data_version` and the notes folders' mtimes (or a 2 s TTL) |
| P3 | Medium | `src/web/instructions/mod.rs:130-155`, `src/agent/taskdir.rs:128-137`, `src/web/agents.rs:72`, `src/web/instructions/list.rs:30-33` | run | S | Load organizations once (`db.list`), skip tasks whose project has no notes folder, read run files only on `/agents` (the instructions page needs counts) |
| P4 | Medium | `src/wiki/ingest_job/mod.rs:104-107`, `src/wiki/ingest.rs:34`, `src/commands/session.rs:141`, `src/agent/solve.rs:240` | run | S | `scope::levels` + `ingest::mode` first; build the `Wiki` only when the mode asks for this trigger |
| P5 | Medium | `src/web/task_wiki.rs:80`, `src/wiki/ingest_job/review.rs:87-101,23-32`, `src/agent/task_prompt.rs:21-34`, `src/web/task_runs.rs:118` | run | M | Review: `in_git`/`changes` once per render (3 spawns → 2), compute `waiting` from the front matter without filling the SPARQL store, or cache the built `Wiki` per notes dir behind its mtimes |
| P6 | Medium | `src/web/mod.rs:78`, `src/web/footer.rs:59`, `src/web/task.rs:64,70`, `src/web/project_edit.rs:65`, `src/web/org_edit.rs:62`, `src/web/task_edit.rs:81`, `src/db/mod.rs:193-206` | run | S | One `Db` per request: a thread-local connection (requests run on one thread under `block_in_place`) or an `App` built by a layer and read from `Cx`; the footer reads from it |
| P7 | Low | `src/web/daily_notes.rs:208-218` | run | S | `prev`/`next` from the file names (`read_dir` + `note_day`) and read the one note; 7.7 ms → ~2 ms for 365 notes |
| P8 | Low | `src/web/task.rs:100-102`, `src/web/task_edit.rs:82-84`, `src/web/org_edit.rs:63-66` | read | S | Badge counts: count `*.yaml` entries without parsing them; `SELECT COUNT(*)` for sessions and projects |
| P9 | Low | `src/push.rs:97-100`, `src/commands/misc.rs:91-108` | read | S | `push::plan`: one `tasks_for_projects` keyed by id instead of two `get`s per completed task; `restore_cmd`: one `list::<Task>`/`list::<Project>` map (the `tmux has-session` spawn per config stays the real cost) |
| P10 | Low | `src/reporting/worklog.rs:52-68` | run | S | Index `tasks` by day once (`HashMap<NaiveDate, Vec<&TaskReport>>`), keep `&Session` instead of cloning; today O(D·S) with clones, measured flat (30 ms for 3200 sessions over 30 days) |

### Details

**P1 · performance · High · `src/agent/run.rs:169-204` · run · S**
`shown_status` calls `alive(pid)` for every record whose status is `running`; `alive` spawns `kill -0 <pid>` and then `ps -o stat= -p <pid>` (`run.rs:197-204`), 5.3 ms a pair on this machine. `state_pill` (`task_runs.rs:189`) renders it once per row on `/agents` (`agents.rs:63`) and on the task Agent tab, and `solve::begin` (`solve.rs:203`) asks `is_running()` of every record until it finds one. With 3 runs per task and run 3 running (one live pid): `/agents` 103 / 418 / 1698 ms at scale 1 / 4 / 16 (`results/routes_loaded_*`), ×16.4 for ×16 tasks; `/task/1?tab=agent` 10.7 ms against 2.1 ms once the record is `done`. Rewriting the 384 records of the scale-16 store from `running` to `done` took `/agents` from 1698 ms to 43 ms: the spawns are 97 % of the page. A real store carries stale `running` records (a crashed agent never writes `ended`), and each costs 5 ms on every render of `/agents` forever, since `kill -0` of a dead pid only skips the `ps`. Derived: Θ(R_running) spawns per render; optimal: one spawn (or none, `kill(2)` through `libc`) per render. Repro: `fsload.sh $SCRATCH/perf16 <server-pid>`, then `curl -w '%{time_total}' http://127.0.0.1:4003/agents`.

**P2 · performance · High · `src/web/search_index.rs:156-160` · run · M**
`results` calls `entries(&open_db()?)` on every request: 4 table scans (`list::<Organization|Project|Board|Task>`) and, through `push_notes` → `notes::list` (`notes.rs:115-128`), a `read_dir`, a `read_to_string` and a YAML front-matter parse of every daily note of every organization and project. The DB part is 4 statements; the file part is Θ(N) reads and parses per keystroke. Measured (`/search?q=` and four other queries, each the same): 18.6 / 80 / 150 ms for 971 / 3884 / 6776 notes (bare seed: 3 / 5.6 / 11.6 ms, so the notes are the whole growth). A year of daily notes in 8 organizations and 64 projects is realistic; 150 ms per keystroke exceeds the interactive budget, and the search script fires on each one. Confirms the seed claim. Optimal: Θ(changed files) per request, i.e. a cached `Vec<Entry>` rebuilt when `PRAGMA data_version` or any notes folder's mtime changes (or every few seconds), then the ranking (already pure, `search.rs`) on the cached entries. Repro: `curl -w '%{time_total}' 'http://127.0.0.1:4003/search?q=task'`.

**P3 · performance · Medium · `src/web/instructions/mod.rs:130-155` · run · S**
`usage(db, None)` lists every task, then for each: `db.get::<Project>` (memoised in a map, fine), `taskdir::of` which runs `db.get::<Organization>` for the project's organization (`taskdir.rs:129-132`, not memoised: an N+1 of n statements), one `read_dir` of `tasks/<id>/runs/` and a `serde_yaml` parse of every record. `/instructions` and `/instruction/{name}` call it only to count uses per instruction (`rows`, `mod.rs:164-170`). Measured 3.6 / 12.0 / 42.4 ms for 30 / 120 / 480 tasks with 3 runs each (×11.8 for ×16), the same for `/instruction/plan`; `/agents` without running records is 43 ms at scale 16, i.e. this walk. Derived Θ(n) statements + Θ(n) `read_dir` + Θ(R) parses; optimal Θ(o) statements and the same file work (the runs are in files, there is no index), or a count of entries for the instructions pages. The statements are ~4 µs each, so the fix is the file side: skip the 20 % of tasks whose project has no notes folder before touching the disk, and parse nothing on the instructions pages.

**P4 · performance · Medium · `src/wiki/ingest_job/mod.rs:104-107` · run · S**
`auto` calls `Target::new` (`mod.rs:48-60` → `scope::wiki` → `Wiki::build`, reading every page and filling the oxigraph store with the entailed facts) and only then `ingest::mode(&target.wiki.levels)`, which needs only the levels (`ingest.rs:34-48` reads `wiki/settings.yaml`). The default mode is `Manual`, so for most users every `session new` (`commands/session.rs:141`), `task done` and `solve begin` (`solve.rs:240`) builds a graph to discard it. Cost of the build, from `wiki find` minus its 31 ms floor: +8 ms at 200 pages, +90 ms at 2000 (`results/wiki.txt`), linear, parallel over up to 8 threads. Confirms the seed claim. Fix: `let levels = scope::levels(db, &spec)?; if mode(&levels) does not want this trigger { return Idle }` then build.

**P5 · performance · Medium · `src/web/task_wiki.rs:80` · run · M**
`ingest_job::review` (`review.rs:87-101`) builds the graph (`Target::new`) and spawns `git rev-parse --is-inside-work-tree`, `git rev-parse --show-prefix` and `git status --porcelain -z -uall -- wiki`, plus one `git diff` per changed page, and reads the lock file, on every render of `/task/{id}?tab=wiki`. Measured on the git-backed store: 36 ms with 2 pages, 43 ms with 200, 122 ms with 2000 (graph ~85 ms + git ~35 ms); on the stores without git 18 ms (one failing `rev-parse`). `Roster::load` with `?new=agent` (`task_runs.rs:118` → `task_prompt::onboarding`, `task_prompt.rs:27`) builds the graph for `retrieve::find`: 2.6 → 9.9 → 93.6 ms at 0 / 200 / 2000 pages. These renders are constant-but-large, linear in W; the CLI equivalents (`wiki pending` 65 → 77 → 165 ms, `wiki find` 31 → 39 → 121 ms) carry the same work once per process, which is inherent. In a `serve` process the graph could be cached per notes folder behind the `md` files' mtimes; short of that, `review` can skip the store fill (`waiting` needs only `source`/`updated` of the front matter, `check::dirty_notes`).

**P6 · performance · Medium · `src/web/mod.rs:78`, `src/web/footer.rs:59` · run · S**
Every page opens the database for its loader and `site_footer_for` opens it again (`footer.rs:59`); `/task/{id}?tab=agent` opens a third for the `App` (`task.rs:70`), and the three edit pages open a third in their screen component (`project_edit.rs:65`, `org_edit.rs:62`, `task_edit.rs:81`). `Db::open` (`db/mod.rs:193-206`) does `create_dir_all`, `Connection::open`, `busy_timeout`, `foreign_keys`, `journal_mode = WAL`, and `migrate()` which is one `PRAGMA user_version` read when up to date (`migrate.rs:11-14`). Measured with Python's sqlite3 doing the same: 0.61 ms per open against 0.004 ms per indexed query. The DB-only pages take 1.2–1.9 ms at scale 16, so two opens are ~0.6–1.2 ms of that, more than all their queries together; `prepare_cached` never hits across requests. The seed claim that each open "runs PRAGMAs, WAL and `migrate()`" is confirmed, the implied cost of `migrate()` is refuted (one pragma read). Re-confirms `review_complexity.md` F8; F1 (footer O(P)) is fixed since (`organizations_with_projects(8)`, one window query, `footer.rs:14`). Fix: one connection per request (thread-local, since `blocking.rs` pins a request to one thread) handed to the footer, or a footer string cached behind `PRAGMA data_version`.

**P7 · performance · Low · `src/web/daily_notes.rs:208-218` · run · S**
`note_page` calls `journal::list(&self.dir)` (reads and parses every note) to find the previous and next day and the note itself, then `journal::raw` reads the same file again. `/org/1/notes/<today>` takes 7.7 ms with 365 notes against 2.4 ms for `/project/1/notes/<today>` with 60. Dates are the file names, so `read_dir` + `note_day` gives the neighbours without reading anything.

**P8 · performance · Low · `src/web/task.rs:100-102` · read · S**
On every tab of `/task/{id}` the Agents badge is `taskdir::of(..).map(|d| d.runs().len())`: an organization lookup, a `read_dir` and a YAML parse per record, for a number. `task_edit.rs:82` counts sessions by reading every row (`sessions_for_task(..).len()`), `org_edit.rs:63` counts projects the same way; `count_tasks_*` already shows the right pattern.

**P9 · performance · Low · `src/push.rs:97-100` · read · S**
`plan` runs `db.get::<Task>` and `db.get::<Project>` for every completed task of the period (2 PK lookups of ~4 µs each; `project_reports` has just read the tasks and projects, so the rows are already in memory minus `github_issue`/`project.github`). `restore_cmd` (`misc.rs:91-108`) does the same two lookups per tracked session config, but its `tmux has-session` spawn per config (`tmux.rs:137`) is three orders larger. Confirms the seed claims as N+1s, with negligible cost.

**P10 · performance · Low · `src/reporting/worklog.rs:52-68` · run · S**
For each day D, every task's session rows are scanned twice (`any` and `max`), O(D·S) with a `Session` clone per row. Measured `org info --from -30d -f worklog`: 30.2 / 30.7 / 30.2 ms at 200 / 800 / 3200 sessions, indistinguishable from the 29 ms process floor. Confirms the shape in the seed claim, refutes any user-visible cost.

### Re-confirmed / fixed since older reports

- `review_complexity.md` F1 (footer Θ(P+O+B) on every page): **fixed since** (`FOOTER_LIMIT = 8`, window query `groups.rs:17-68`), the second open remains (P6).
- `review_complexity.md` F2/F3/F4 (report, tasks tab and boards read every row): **re-confirmed as design**, measured harmless: `/org/1?tab=report` 2.6 ms, `/board/1` 2.4 ms (74 KB), `/org/1?tab=tasks` 2.3 ms at scale 16.
- `review_complexity.md` F8 (per-request connection set-up): **re-confirmed** (P6), now with a number.
- `web_final_review.md` C1 (`project_reports` N+1): **fixed since** (`utils/report.rs:103-135`, two `json_each` queries; plans use `idx_sessions_task_start`).
- `web_final_review.md` C2 (blocking SQLite on tokio workers): **fixed since** via `blocking.rs`; not re-measured under concurrency.

## Table of routes and commands

Times are medians in ms on the loaded stores (`results/routes_loaded_*`), scale 1 / 4 / 16; "q" = SQL statements, "opens" = `Db::open` calls, FS = file work. Bare-seed numbers for the DB-only pages were 1.5–4 ms with the same ratios.

| Route / command | Derived cost per request | Optimal | ×1 | ×4 | ×16 | ×16/×1 |
|---|---|---|---|---|---|---|
| `/` | 2 opens, 2 q (`list` projects, orgs) + footer 3 q; Θ(p) rows | Θ(p) | 1.3 | 1.4 | 1.7 | 1.31 |
| `/settings` | 2 opens, footer only | O(1) | 1.1 | 1.2 | 1.4 | 1.27 |
| `/org/{id}` (info) | 2 opens, 3 q (get, projects_in, count) + footer; `notes::count` read_dir | O(1)+Θ(p_org) | 1.6 | 1.7 | 1.9 | 1.19 |
| `/org/{id}?tab=notes` | + `notes::list`: Θ(N_org) reads+parses, 132 KB page | Θ(N_org) | 9.1 | 9.0 | 9.7 | 1.07 |
| `/org/{id}?tab=tasks` | + `tasks_in` (2 q), filter in Rust Θ(n_org) | Θ(n_org) | 1.7 | 1.8 | 2.3 | 1.35 |
| `/org/{id}?tab=report` | + 2 q (`sessions_for_projects_in_range`, `tasks_for_projects`), Θ(n_org + s_range) | Θ(s_range) | 1.9 | 2.0 | 2.6 | 1.37 |
| `/org/{id}?share=1` | as info (dialog needs `tab=report`) | — | 1.6 | 1.7 | 1.9 | 1.19 |
| `/org/{id}/edit` | **3 opens**, 4 q (get, list projects, projects_in, count) | 2 q | 3.1 | 3.2 | 3.5 | 1.13 |
| `/org/{id}/notes/{date}` | 2 opens, 1 q + `notes::list` Θ(N_org) + raw read (P7) | O(1) + read_dir | 7.7 | 7.8 | 8.0 | 1.04 |
| `/org/{id}/platform-activities` | 1 open, 1 q, outbound call (dummy key: none) | O(1) | 1.9 | 1.9 | 1.9 | 1.00 |
| `/project/{id}` | 2 opens, 3 q + footer, `notes::count` | O(1) | 1.4 | 1.5 | 1.6 | 1.14 |
| `/project/{id}?tab=notes` | + `notes::list` Θ(N_proj) | Θ(N_proj) | 2.6 | 2.7 | 2.9 | 1.12 |
| `/project/{id}?tab=tasks` | + `tasks_for_project` Θ(n_proj) | Θ(n_proj) | 1.4 | 1.5 | 1.7 | 1.21 |
| `/project/{id}/edit` | **3 opens**, 3 q | 2 q | 1.6 | 1.7 | 1.8 | 1.12 |
| `/project/{id}/task/new` | 303 redirect, 0 q | O(1) | 0.4 | 0.4 | 0.4 | 1.00 |
| `/project/{id}/notes/{date}` | as org note, Θ(N_proj) | O(1) | 2.4 | 2.5 | 2.7 | 1.13 |
| `/task/{id}` (info/sessions/notes) | 2 opens, 6 q (task, project, org, sessions, tags, org again via `taskdir::of`) + `runs()` parse (P8) + footer | 5 q | 1.5 | 1.6 | 1.9 | 1.27 |
| `/task/{id}?tab=agent` | **3 opens**, +3 org lookups (`taskdir::of` ×2, `layers_of`, `in_notes`), `runs()` ×2, `kill`+`ps` per running run (P1) | 1 run walk, 0 spawns | 10.7 | 10.9 | 11.5 | 1.07 |
| `/task/{id}?tab=agent&new=agent` | + `Wiki::build` of task scope Θ(W) + `instruction::list` (P5) | cached graph | 17.2 | 17.5 | 16.6 | 0.97 |
| `/task/{id}?tab=wiki` | + `Target::new` Θ(W) + 3 `git` spawns (+1 per change) (P5) | cached graph, 1–2 spawns | 18.4 | 18.6 | 18.4 | 1.00 |
| `/task/{id}/edit` | **3 opens**, 5 q + `sessions_for_task` for a count (P8) | 4 q | 1.7 | 1.9 | 2.1 | 1.24 |
| `/agents` | 2 opens, 1 + n q (P3), n read_dir, R parses, 2 spawns per running run (P1) | o+1 q, 1 spawn | 103 | 418 | **1698** | **16.4** |
| `/boards` | 2 opens, 2 q | Θ(p) | 1.2 | 1.5 | 1.6 | 1.33 |
| `/board/{id}` (agenda, `?date`, `?pick`) | 2 opens, 5 q (board, colors, projects_in, tasks, priorities), sort Θ(n_board log) | 5 q | 1.6 | 2.0–2.3 | 2.4–2.6 | 1.5–1.6 |
| `/board/{id}/matrix` | same 5 q | 5 q | 1.5 | 1.9 | 2.3 | 1.53 |
| `/board/{id}/info` | 2 opens, 2 q | 2 q | 1.3 | 1.8 | 2.2 | 1.69 |
| `/board/{id}/edit` | 2 opens, 2 q (`load_among`, `project_owners`) | 2 q | 1.3 | 1.5 | 1.7 | 1.31 |
| `/instructions`, `/instruction/{name}` | 2 opens, `usage()` 1 + n q, n read_dir, R parses (P3) | o q, counts only | 3.6 | 12.0 | **42.4** | **11.8** |
| `GET /search?q=` (5 queries) | 1 open, 4 q, Θ(N) file reads + parses, Θ(E·|q|) ranking (P2) | cached entries | 18.6–21 | 78–81 | **131–150** | **7–8** |
| static assets | 0 q, ETag | — | 0.4–0.5 | 0.4–0.5 | 0.4–0.5 | 1.0 |
| CLI `project` / `task` (list) | 1 open, 1 q | — | 29.6 | 29.3 | 29.2–30.3 | 1.0 |
| CLI `project info` (day / 30 d) | 1 open, 3 q | — | 30.6 / 32.1 | 29.9 / 29.2 | 29.4 / 29.7 | 1.0 |
| CLI `org info` (day / 30 d / worklog) | 1 open, 4 q, Θ(n_org + s_range), worklog O(D·S) (P10) | — | 30.3 / 30.2 / 30.2 | 29.1 / 30.5 / 30.7 | 29.8 / 31.4 / 30.2 | 1.0 |
| CLI `session start` + `stop` (2 processes) | 2 opens, 3 + 3 q | — | 38.3 | 40.7 | 38.4 | 1.0 |
| CLI `notes list --org` / `--project` | 1 open, 1–2 q, Θ(N) reads (titles are in the files) | Θ(N) | 36.8 / 30.0 | 40.6 / 30.8 | 36.8 / 32.1 | 1.0 |
| CLI `wiki find/show/check --org|--task` | 1 open, 2–4 q, `Wiki::build` Θ(W), `find` Θ(W·|q|), `check` Θ(W + edges) | inherent per process | 34–37 | 35–37 | 34–40 | 1.0 |
| CLI `wiki pending --project` | + 3–4 `git` spawns | 2 spawns | 46.3 | 49.6 | 46.8 | 1.0 |

Wiki size (scale-1 store, org-1 wiki with 2 / 200 / 2000 pages, `results/wiki.txt`): `wiki find` 31 / 39 / 121 ms, `wiki check` 31 / 42 / 164 ms, `wiki export` 32 / 39 / 132 ms, `wiki pending` 65 / 77 / 165 ms, `/task/1?tab=agent&new=agent` 2.6 / 9.9 / 93.6 ms, `/task/1?tab=wiki` 36 / 43 / 122 ms. Ratio 2000/200 ≈ 10–11 on the graph part: linear in W. The CLI floor is ~29 ms of which ~23 ms is the Python timing harness (row `baseline python-spawn`) and ~6 ms the binary's start-up (`--version`).

## Checked and found optimal

- **Every statement in `src/db` uses an index** (`EXPLAIN QUERY PLAN`, scale-16 file): `tasks(project_id[,name])` via `sqlite_autoindex_tasks_1` (covering), `sessions` via `idx_sessions_task_start` (task_id and range), `projects(organization_id|board_id)`, `session_configs(task_id)` and `(tmux_session_name)`, `task_tags` both ways, `tags(name)`, `pushes(organization_id, hash)`; `json_each` lists become bloom-filtered subqueries. The only `SCAN`s are intentional full lists (`list::<T>`, `task_refs`).
- `project_reports` / `org info` / `/org?tab=report`: two batched queries however many projects (`utils/report.rs:103-135`), range-bounded on the index.
- Footer: bounded to 8 rows per group by one window-function query (`groups.rs:17-68`, `first_by_name`).
- Board pages: 5 statements independent of n; sort once (`load.rs:52-56`); priorities one join.
- Task filter (`task_query.rs:136-154`): one tag query only when a `tag:` term is present, Θ(n) matching.
- `task_refs` for completion: one join, indexed.
- `session start/stop`: 3 indexed statements each; `resolve_task` 2 lookups.
- `Wiki::build` (`graph.rs:179-243`): lists files first, reads them on up to 8 threads, `HashSet` dedup, `HashMap` index; `Wiki::resolve` O(L) hash lookups; `fill` collects facts then one `store.extend` (not one transaction per insert); `entail` visits each derived fact once (transitive closure bounded by the `in` chain of L ≤ 3 levels and `sourceOf` chains). Measured linear: +8 ms / 200 pages, +90 ms / 2000.
- `retrieve::find`: Θ(W·|words|) string scans + one sort; `check`: Θ(W) node pass + Θ(W) `edges_out` (each an indexed quad pattern) + Θ(W) `md_files` for strays; both linear in the measurement.
- `migrate()` after the first open: one `PRAGMA user_version` read (`migrate.rs:11-14`); the seed finding's implied cost does not hold.
- `blocking.rs` `block_in_place` per request: correct for blocking SQLite; not a scaling factor for one client.
- `notes::count` for badges: `read_dir` only, no reads.
- Static assets: compiled in, ETag, 0.4 ms.

## Confidence

- High on P1, P2, P3 (clean linear measurements with one cause each, cause isolated by removing it: P1 by rewriting the records, P2/P3 by the bare-seed vs loaded comparison).
- High on P4 (read; the build cost it incurs is measured, the control flow is explicit).
- Medium on P5's absolute numbers (git spawn time varies with the repository) and on P6's 0.6 ms per open (Python sqlite3 stand-in for rusqlite; the direction and order are certain, the constant may differ by ±30 %).
- High that nothing in scope is super-linear: every ratio for ×16 data is ≤ ×16.4 and the one at ×16.4 is linear in a count that the seed grew ×16 (running records).
- CLI numbers are dominated by the harness (23 ms) and process start-up; differences under 3 ms between commands are noise.

## Out-of-scope observations

Security (always reported):
- `/agents` renders every run's `model`/`effort` and the raw `started` string from YAML files an agent can edit (`agent/run.rs`, `--add-dir <taskdir>` with `acceptEdits`); `alive(pid)` and `interrupt(pid)` take the pid from the same file, so a run that writes `pid: 1` makes `/agents` probe `launchd` and Cancel signal it (`kill -TERM -- -1` would be every process of the user). Confirms the seed's `pid: 0` concern from the performance angle too: a stale `pid` costs 5 ms per render forever.
- `/search` reads every note file of every notes folder on an unauthenticated local GET; a bystander on the same host with a browser can enumerate note titles and bodies through `/search?q=note` (guard is local-only, same as every page).
- `/task/{id}?tab=wiki` spawns `git` in the notes directory on a GET; a `.git` folder with hooks there executes them.

Other:
- `/agents` is 346 KB at scale 16 (every run of every task, no limit, `usage()` has no cap unlike `RUNS = 50` in `task_runs.rs:51`).
- `/org/{id}?tab=notes` is 132 KB with a year of notes: the preview strings are truncated but every note is listed.

## What I could not assess

- Concurrency: one client at a time; `busy_timeout` stalls, lock contention between `serve` and the CLI, and `block_in_place` behaviour under many simultaneous `/search` requests were not measured.
- The outbound platform call of `/org/{id}/platform-activities` and `push` (dummy key; no platform).
- tmux-driven `session new`, `restore` and the ingest agent run itself (spawns `claude`); their cost is dominated by the external processes.
- Browser-side cost of the 74–346 KB pages (DOM size, the agenda's drag handlers).
- Exact rusqlite open cost (approximated with Python's sqlite3).
