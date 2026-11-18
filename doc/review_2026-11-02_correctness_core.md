# Correctness review, core (CLI, agent, wiki, db, push) — 2026-11-02

Lens: `correctness_core`. Scope: everything under `src/` except `src/web/`, plus `macros/`. Branch `feat/review` at `5d96f44`, release binary `target/release/Iter` of HEAD.

## Summary

The core is in good shape: every byte-slicing site I checked is on an ASCII boundary, the worklog is day-sorted by construction, indexes and the `status` CHECK exist on a fresh store, and `cargo test --no-default-features` passes (370 tests, 0 failed, 2 ignored). What is wrong clusters around edges the code does not guard: filesystem recursion, names that look like flags, time arithmetic in naive local time, and a few places where two processes or two halves of a write can disagree. Nothing is a Blocker. Nineteen findings: 1 High, 10 Medium, 8 Low. Five seed findings are refuted with evidence (listed in "Refuted").

Top 5:

1. **C1 (High, run)** `iter clone <project>` with a `base_path` inside the source copies the source into itself until `ENAMETOOLONG` (176 nested directories in my run), leaves them behind and creates no project (`src/scaffold.rs:26`, `src/commands/project.rs:100`).
2. **C2 (Medium, run)** The tmux hooks pass the session name positionally; a project or task name starting with `-` makes `iter internal hook` fail with clap exit 2, so that session's clock is never started or stopped (`src/args/internal.rs:29-38`, `src/commands/session.rs:197`).
3. **C3 (Medium, run)** `iter task done` without `--save` deletes the task branch with `git branch -D` even when it holds commits reachable from nowhere else; documented in README, but there is no check or warning, and the commit survives only in the reflog (`src/git.rs:529`, `src/utils/workspace.rs:321`).
4. **C4 (Medium, read)** The wiki lock has a check-then-remove window: two processes that both find a stale `wiki.lock` can both take it, and the first to finish removes the other's lock (`src/wiki/ingest_job/lock.rs:39-44`).
5. **C5 (Medium, run)** The CLI stores a task name as typed (`'task '`), the web form trims it; `cc-src/task` then does not resolve and a second task `task` is created beside it (`src/commands/task.rs:172`, `src/models/mod.rs:490`).

## Scope

Read in full: `push.rs`, `platform.rs`, `agent/{plan,run,solve,taskdir,exec,task_prompt,instruction(part),prompt(part)}.rs`, `wiki/ingest_job/*`, `wiki/{ingest,scope,check(part),mv(part),frontmatter(part),graph(part),ontology(part)}.rs`, `commands/{task,project,session,misc,sync,organization,board,wiki,ingest}.rs`, `db/{mod,migrate,tasks,sessions,reporting,pushes,groups,sqltypes}.rs`, `models/*`, `reporting/{mod,worklog,merge}.rs`, `utils/{clock,report,resolve,workspace,crud,output}.rs`, `config.rs`, `tmux.rs`, `git.rs`, `github.rs`, `process.rs`, `md_edit.rs`, `notes.rs`, `scaffold.rs`, `secret.rs`, `completion.rs`, `main.rs`, `app.rs`, `macros/src/lib.rs`.
Skimmed or grep-checked only: `wiki/retrieve.rs`, `wiki/schema.rs`, `reporting/render.rs`, `error.rs`, `agent/error.rs`, `fixtures.rs`, the test modules.
Not reviewed: `src/web/` (other lens); the prompt templates under `wiki/prompts/` and `agent/instructions/`.

## Method

- Environment per the brief: `XDG_CONFIG_HOME` pointed at scratch homes only, `ITER_PLATFORM_KEY=dummy`, `TMUX` unset; the live store under `~/.config/iter` was never opened.
- `code-review` skill, `medium src/ --max-findings all`: it accepted the path target and returned 18 findings (JSON). Each in-scope finding was re-verified against the source before being kept; one was refuted (see "Refuted", R6) and the `src/web/` ones are listed under out-of-scope observations.
- Manual read of the hunt list from the task brief, with `grep` for byte slicing (`[..n]`, `[n..]`, `split_at`), `unwrap/expect`, `remove_dir_all/remove_file`, `Local::now`.
- Run evidence with the release binary against a scratch store (`$SCRATCH/cc-home`), using a scripted editor (`doc/review_tools/correctness_core/ed.sh`: applies `$ITER_ED_SED` to the buffer iter opens). Every run finding below is reproducible with `doc/review_tools/correctness_core/repro.sh <scratch-home> $BIN`.
- `sqlite3` on the seeded scale-4 store (`$SCRATCH/iter-home/iter/iter.db`) for schema facts; a throwaway `tmux` server (`TMUX_TMPDIR=/tmp/itx.*`) for target matching.
- `XDG_CONFIG_HOME=$SCRATCH/test-home cargo test --no-default-features 2>&1 | tail -30`, verbatim tail:

```
test wiki::mv::tests::bad_targets_are_refused ... ok
test wiki::ingest_job::tests::a_finished_tasks_notes_go_into_todays_note_once ... ok
test wiki::ontology::tests::an_enumerated_range_lists_the_allowed_values ... ok
test wiki::ontology::tests::an_extension_adds_a_class_with_its_place ... ok
test wiki::ontology::tests::a_bad_place_is_an_error ... ok
test wiki::ontology::tests::pages_are_the_classes_under_page ... ok
test wiki::ontology::tests::the_built_in_ontology_says_where_classes_live ... ok
test wiki::ontology::tests::relation_keys_are_the_authorable_properties ... ok
test wiki::ontology::tests::the_rules_are_read_from_the_owl ... ok
test wiki::mv::tests::moving_a_page_rewrites_what_points_at_it ... ok
test wiki::retrieve::tests::find_ranks_by_tag_path_and_description ... ok
test wiki::retrieve::tests::find_stops_at_the_budget_but_keeps_the_best ... ok
test wiki::retrieve::tests::grep_finds_lines_in_pages_and_in_notes_on_request ... ok
test wiki::schema::tests::an_extension_shows_up ... ok
test wiki::retrieve::tests::show_goes_deeper_with_the_depth ... ok
test wiki::retrieve::tests::ids_are_bare_unless_the_page_is_in_a_level_the_question_cannot_see_into ... ok
test wiki::schema::tests::the_tables_come_from_the_ontology ... ok
test wiki::retrieve::tests::neighbors_walk_the_graph_nearest_first ... ok
test wiki::scope::tests::a_project_without_an_organization_is_its_own_scope ... ok
test wiki::scope::tests::nowhere_to_keep_notes_is_an_error ... ok
test wiki::scope::tests::an_organization_goes_down_into_its_projects_only_when_asked ... ok
test wiki::scope::tests::a_task_sees_itself_its_project_and_its_organization ... ok
test wiki::ingest_job::tests::only_one_agent_holds_the_wikis_lock ... ok
test wiki::ingest_job::tests::pending_pages_are_found_with_their_diffs_in_a_repository_above_the_notes ... ok
test wiki::ingest_job::tests::throwing_away_touches_the_pages_and_nothing_else ... ok
test wiki::ingest_job::tests::accepting_commits_the_pages_only ... ok

test result: ok. 370 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 1.08s
```

What was NOT checked: anything needing `gh` or a real time platform (the `task pull`/`push` and `org push` network paths were read, not run); the agent harness itself (`claude` was never spawned); DST behaviour at a real clock change (reasoned from the code, see C7); the web half of every shared module.

## Findings

| ID | Severity | file:line | Evidence | Effort | One-line fix |
|----|----------|-----------|----------|--------|--------------|
| C1 | High | `src/scaffold.rs:26`, `src/commands/project.rs:100-106` | run | S | Refuse a `dest` that is inside `src` (canonicalize both, `starts_with`) before `create_dir_all`; also skip `.iter-worktrees`. |
| C2 | Medium | `src/args/internal.rs:29-38`, `src/commands/session.rs:197`, `src/tmux.rs:293` | run | S | `#[arg(allow_hyphen_values = true)]` on the hook positionals, or emit `--` before the format variables in `hook_action`; and strip a leading `-` in `tmux_name_part`. |
| C3 | Medium | `src/git.rs:529`, `src/utils/workspace.rs:321`, `src/commands/task.rs:307` | run | M | In `task done` without `--save`, check `git branch --contains`/`merge-base --is-ancestor` first and refuse (or warn, keep the branch) when the branch has commits not on `default_branch`. |
| C4 | Medium | `src/wiki/ingest_job/lock.rs:39-44` | read | M | Take the lock with `flock` on a long-lived file (as `taskdir.rs:481` does) instead of `create_new` + stale-pid removal, or re-check ownership (pid in file) in `Drop` before removing. |
| C5 | Medium | `src/commands/task.rs:172`, `src/models/mod.rs:490`, `src/commands/sync.rs:503` | run | S | Trim `edit.task.name` in `checked_tags` (the `Named` derive already trims projects/orgs/boards/tags, `macros/src/lib.rs:118`); trim the issue title in `task_pull` too. |
| C6 | Medium | `src/reporting/worklog.rs:52-54`, `src/db/reporting.rs:412,441` | run | M | Either split a session at midnight when bucketing by day (and in the range query) or state in the report that a session belongs to its start day. |
| C7 | Medium | `src/utils/clock.rs:18,35`, `src/models/session.rs:473,483` | read | M | Store sessions in UTC (or with offset) and convert for display; at minimum do not treat a negative `end - start` as "under a minute" in `too_short`. |
| C8 | Medium | `src/tmux.rs:153,197` | run | S | Use `exact(session)` (`=name`) in `any_client_attached` and `take_detach_message` like every other `-t`. |
| C9 | Medium | `src/commands/organization.rs:124-137` | read | S | Restore echo in a guard that also runs on `SIGINT` (install a handler or use a crate like `rpassword`), not only on the normal path. |
| C10 | Medium | `src/platform.rs:252-264` | read | S | Only treat a reply that parses to a `Results` with a `results` key as authoritative; a 2xx body without it should be an error like non-JSON is. |
| C11 | Medium | `src/platform.rs:76`, `src/push.rs:207,257,362` | read | S | Normalise the platform's `date`/`day` through `NaiveDate::parse_from_str` (accept RFC 3339 too) before comparing with `%Y-%m-%d`. |
| C12 | Low | `src/commands/task.rs:161-162,205-206` | read | S | Wrap insert/update + `set_task_tags` in `db.atomically`, or make `Db::save_task` available without the `web` feature (re-confirms Oct review DRY F1). |
| C13 | Low | `src/push.rs:367-370,410`, `src/models/push.rs:36-44` | read | S | Replace newlines in `result.status`/`result.message` and in `e` before writing the ledger line (as `conflict_line` does). |
| C14 | Low | `src/wiki/ingest_job/review.rs:44-48,130` | read | S | Classify any code whose first letter is `A` (e.g. `AM`) as `new`, or run `git reset` before computing the list in `settle(revert)`. |
| C15 | Low | `src/wiki/ontology.rs:217-233` | read | S | Bound the `rdf:List` walk (visited set or max length). |
| C16 | Low | `src/wiki/check.rs:80-82`, `src/wiki/ingest_job/mod.rs:116` | read | S | Treat a page without `updated` as "ingested at its mtime", or lint it; otherwise its source note is dirty forever and every trigger spawns an ingest. |
| C17 | Low | `src/commands/misc.rs:143` | read | S | Warn and continue on a `create_session` failure in `restore_cmd` so hooks are installed and clocks closed for the other sessions. |
| C18 | Low | `src/wiki/ingest_job/spawn.rs:15-17` | read | S | Take the lock before creating the log file; a busy ingest currently leaves an empty `.iter/ingest-*.log` behind. |
| C19 | Low | `src/db/migrate.rs:187-190,135` | run | S | Fix the comment: indexes are created only in `baseline`; a later index on a later column needs its own migration step. |

### C1 · correctness_core · High · `src/scaffold.rs:26`, `src/commands/project.rs:100-106` · run · S

`clone_cmd` creates `dest` (`create_dir_all`) and then `copy_dir_excluding_git(src, dest)`. `read_dir(src)` sees the freshly created `dest` when it lies under `src`, recurses into it, and copies the growing destination into itself. The loop ends only when the path exceeds `PATH_MAX`; the directories stay, and `db.insert` is never reached. Nothing checks that `dest` is outside `src`; `.iter-worktrees/` is copied as well (seed confirmed).

Repro (scratch store, scripted editor):
```
ITER_ED_SED="s|^name:.*|name: clone1|; s|^base_path:.*|base_path: $SRC/sub/clone|" $BIN clone cc-src
# → error: I/O error: File name too long (os error 63)
find $SRC/sub/clone -type d | wc -l   # 176 ; deepest path has 185 components ; no 'clone1' row in projects
```

### C2 · correctness_core · Medium · `src/args/internal.rs:29-38`, `src/commands/session.rs:197` · run · S

`tmux_name_part` flattens `/ : . * ?` but keeps a leading `-`; `hook_action` writes `... internal hook session-closed #{q:hook_session_name}` without `--`, and the `HookEvent` positionals do not allow hyphen values. tmux passes the name as one argument, clap rejects it, the hook does nothing, and the task's clock is neither started (`client-session-changed`) nor stopped (`session-closed`, `client-detached`). `session new` for a project named `-web` is accepted (`Project::check` only forbids `,` and `/`).

Repro:
```
$BIN internal hook session-closed -web/a
# error: unexpected argument '-w' found ... exit=2
$BIN internal hook client-session-changed /dev/ttys1 -web/a ""   # exit=2
```

### C3 · correctness_core · Medium · `src/git.rs:529`, `src/utils/workspace.rs:321`, `src/commands/task.rs:307` · run · M

`teardown_session_config` always runs `git worktree remove --force` then `git branch -D`. README 478-494 says so and offers `--save`, but a plain `task done` on a branch with unmerged commits deletes it silently; the commit then exists only as an unreachable object (reflog, 30 days by default). The CLI prints git's own "Deleted branch feat/feat1 (was 0e57a10)" and nothing else. `task delete` and `project delete` take the same path.

Repro:
```
$BIN session new cc-repo/feat1 ; (cd <worktree> && echo work > work.txt && git add . && git commit -m unmerged)
$BIN task done cc-repo/feat1
# Deleted branch feat/feat1 (was 0e57a10).  task 'cc-repo/feat1' marked done
git branch --contains 0e57a10   # (nothing) ; git cat-file -t 0e57a10 → commit (dangling)
```

### C4 · correctness_core · Medium · `src/wiki/ingest_job/lock.rs:39-44` · read · M

`lock()` does `create_new`; on `AlreadyExists` it calls `busy()` (reads the pid, `kill -0`) and, if not busy, `remove_file` and retries. Two processes arriving at a stale lock together both see "not busy", A removes and recreates, B then removes **A's** new lock and creates its own: both run agents on the same wiki. `busy()` also returns `false` for a file that exists but is still empty (the writer is between `create_new` and `write!`), so even a live lock can be removed in that window. Finally `Drop` removes the path unconditionally, so when A finishes it deletes B's lock. The `auto()` trigger runs on every `session new`, `task done` and `solve begin`, from CLI and web alike, so two triggers close together are realistic.

### C5 · correctness_core · Medium · `src/commands/task.rs:172`, `src/models/mod.rs:490` · run · S

`checked_tags` calls `require_name`, which only tests `trim().is_empty()`; the stored name keeps its whitespace. The `Named` derive trims (`macros/src/lib.rs:118`, "`\" work \"` would be stored as typed and never found") and the web form trims (`src/web/forms/task.rs:123`); tasks created or edited from the CLI are the exception. `task_pull` (`src/commands/sync.rs:503`) also stores `issue.title` without `require_name` (seed confirmed), so an issue with an empty title becomes a task named `""`.

Repro:
```
ITER_ED_SED="s|^name:.*|name: 'task '|" $BIN task new cc-src   # created task 'cc-src/task '
$BIN task info cc-src/task        # error: no such task 'task' in project 'cc-src'
ITER_ED_SED="s|^name:.*|name: task|" $BIN task new cc-src      # created task 'cc-src/task' (second row)
```

### C6 · correctness_core · Medium · `src/reporting/worklog.rs:52-54`, `src/db/reporting.rs:412,441` · run · M

Sessions are selected and bucketed by `start`. A session 23:30 → 01:30 is reported as 2 h on its start day and 0 h on the next, and the worklog pushed to the platform logs 2 h on the first day. Users working past midnight see a wrong daily total on both days.

Repro:
```
sqlite3 $DB "insert into sessions(task_id,start,end) select id,'2026-10-20 23:30:00','2026-10-21 01:30:00' from tasks limit 1"
$BIN project info cc-src --date 2026-10-20 -f yaml | grep total_hhmm   # 02:00
$BIN project info cc-src --date 2026-10-21 -f yaml | grep total_hhmm   # 00:00
```

### C7 · correctness_core · Medium · `src/utils/clock.rs:18,35`, `src/models/session.rs:473,483` · read · M

Sessions are stored as `Local::now().naive_local()` and durations are naive differences. On the autumn change (Denmark: 2026-10-25, 03:00 → 02:00) a session started at 02:50 and stopped 30 real minutes later reads 02:20: `too_short` sees `-30 min < 60 s` and `close_open_session_at` **deletes** it (`clock.rs:51-54`); a session 01:30 → 02:15 across the change (1 h 45 real) counts 45 min. On the spring change the opposite hour is over-reported. Once a year, but silent and destructive on the delete path.

### C8 · correctness_core · Medium · `src/tmux.rs:153,197` · run · S

Every other target goes through `exact()` (`=name`); `any_client_attached` and `take_detach_message` pass the bare name, and tmux resolves a bare `-t` by prefix. In `hook_cmd` the `client-session-changed` hook calls `client_left_tmux_session(previous)` where `previous` may already be gone (killed while attached): `list-clients -t web/a` then matches `web/ab`, returns `true`, and the clock stays open (the `session-closed` hook is the only backstop). `restore_cmd` does the same for sessions it could not recreate and may start the wrong task's clock.

Repro (throwaway server):
```
TMUX_TMPDIR=$(mktemp -d /tmp/itx.XXXX) tmux new-session -d -s web/ab
tmux has-session -t web/a   ; echo $?   # 0  (matched web/ab)
tmux has-session -t =web/a  ; echo $?   # 1  can't find session: web/a
tmux display-message -p -t web/a '#{session_name}'   # web/ab
```

### C9 · correctness_core · Medium · `src/commands/organization.rs:124-137` · read · S

`read_secret` runs `stty -echo`, blocks in `read_line`, then `stty echo`. Ctrl-C during the prompt terminates the process by the default `SIGINT` action before the second `stty`, leaving the user's terminal without echo until they type `reset`. Seed confirmed; no guard or signal handling anywhere in the binary.

### C10 · correctness_core · Medium · `src/platform.rs:252-264` · read · S

`Results` has `#[serde(default)] results`, so any 2xx JSON object without a `results` key (`{}`, `{"error":"quota exceeded"}`) parses to an empty list, and the loop at 256-264 back-fills every entry as `ok`. The comment above says only an empty body means "all accepted"; the code accepts any JSON object. The ledger then records the days as pushed and `execute` closes the GitHub issues of completed tasks. (The existing test `entries_a_response_does_not_mention_were_accepted` enshrines the `{}` case, so this is a deliberate but risky choice.) From the code-review skill, verified.

### C11 · correctness_core · Medium · `src/platform.rs:76`, `src/push.rs:207,257,362` · read · S

`WireEntry.date` and `EntryResult.day` are copied verbatim and compared by string equality with `fmt_date` output. A platform spelling the date `2026-09-04T00:00:00Z` or `2026-9-4` disables the conflict check in `check()` (so the day is pushed again) and, in `execute()`, makes every result unmatched: the entry is back-filled as `ok` (C10) and the foreign `result.day` is recorded as "pushed 0.00 h". Seed "push/worklog matching by day" confirmed in this form; `Plan::days` itself is fine (see R1). From the code-review skill, verified.

### C12 · correctness_core · Low · `src/commands/task.rs:161-162,205-206` · read · S

`db.insert(&edit.task)` and `db.set_task_tags(id, ...)` are separate transactions; a failure in the second (busy database, 5 s timeout) leaves a tagless task while the editor flow reports an error, and the retry hits `NameTaken`. The web path uses `Db::save_task` inside `atomically`, but it is `#[cfg(feature = "web")]`. Re-confirmed from the 2026-10-01 DRY review (F1), still open.

### C13 · correctness_core · Low · `src/push.rs:367-370,410`, `src/models/push.rs:36-44` · read · S

The ledger `results` column is newline-delimited and parsed by line (`ok_items`, `complete`). `conflict_line` strips newlines from the platform's note, but the `FAILED: platform: <day>: <status>: <message>` line and the GitHub error line do not; a message containing `\nok: platform: 2026-09-04 1.00 h` is read back on the next run as a day pushed earlier. Requires a hostile or buggy platform. From the code-review skill, verified.

### C14 · correctness_core · Low · `src/wiki/ingest_job/review.rs:44-48,130` · read · S

Only `??` and `A` are "new"; a staged-then-edited new page (`AM`, left by an `accept` whose commit failed) is "changed", and `settle(revert)` runs `git checkout HEAD -- <path>` on a path HEAD does not have, which fails and aborts the revert with earlier files already reverted. From the code-review skill, verified.

### C15 · correctness_core · Low · `src/wiki/ontology.rs:217-233` · read · S

`allowed_values` walks `rdf:first`/`rdf:rest` until `rdf:nil` with no visited set; a cyclic list in a user's `wiki/ontology.ttl` makes every wiki command spin forever. From the code-review skill, verified.

### C16 · correctness_core · Low · `src/wiki/check.rs:80-82`, `src/wiki/ingest_job/mod.rs:116` · read · S

`dirty_notes` marks a note `Changed` whenever none of the pages made from it carries a parseable `updated`. An agent that omits `updated` (the prompt asks for it, but `check` does not lint its absence) keeps its source note dirty forever, so every `session new`/`task done` with `ingest: on_task_*` spawns another background agent.

### C17 · correctness_core · Low · `src/commands/misc.rs:143` · read · S

`tmux.create_session(name, &cwd)?` aborts `restore_cmd` on the first failure; hooks are then not reinstalled and no stale clock is closed for the other tracked sessions.

### C18 · correctness_core · Low · `src/wiki/ingest_job/spawn.rs:15-17` · read · S

`run_agent` creates `.iter/<what>-<stamp>.log` before `lock(target)?`; when the wiki is busy the empty log stays.

### C19 · correctness_core · Low · `src/db/migrate.rs:187-190,135` · run · S

The doc comment says an index "appears on the first run the schema can take it", but `create_indexes` is called only from `baseline` (migration 0), which runs once. Today that is harmless: `add_missing_columns` runs before it, and on the seeded store all five indexes exist (`sqlite3 ... "SELECT name FROM sqlite_master WHERE type='index'"` lists `idx_sessions_task_start`, `idx_projects_organization`, `idx_projects_board`, `idx_session_configs_tmux`, `idx_task_tags_tag`, `idx_pushes_organization`). A future index on a later column will silently never be created on upgraded databases.

## Refuted seeds and skill claims

- **R1** `Plan::days` sorted assumption (`src/push.rs:125-133`): `Worklog::build` collects entries from a `BTreeMap<NaiveDate, _>` (`src/reporting/worklog.rs:52-57`), so entries are day-ascending by construction; `first`/`last` are correct. read.
- **R2** `plan.rs:344` byte slicing of multibyte chars: `line.find('[')` returns the byte index of an ASCII char and `open + 2` skips the ASCII mark char that `step()` already validated; the line's leading whitespace may be non-ASCII but lies before the index. Run: a plan with `\u{a0}- [ ] {s1} 設計 (verify)` and `ünïcödé —` saved (2 steps), `--mark s1 done` succeeded, `--show -f json` round-tripped the text.
- **R3** `TaskStatus` unknown → `Queue` rewrite (`src/db/sqltypes.rs:620`): the `tasks` table has `CHECK (status IN ('queue','wip','done'))` (sqlite3 on the seeded store), so the fallback is unreachable for stored rows. read+run.
- **R4** Indexes never created after baseline: all five exist on a fresh store (see C19); only the comment is wrong. run.
- **R5** Config `OnceLock` leak (`src/config.rs:166-190`): one `Box::leak` per `replace`, which only the settings page calls; bounded and documented. Not a finding.
- **R6** Code-review skill claim at `src/commands/organization.rs:226` ("`--force` on a complete push inserts an all-FAILED row and `already_pushed` is false forever"): `execute()` returns before the ledger insert when `done_days` is empty (`src/push.rs:374-376`), so no row is written. Refuted by read.
- **R7** Code-review skill claim at `src/wiki/ingest_job/review.rs:34` (rename old-name record shorter than 4 bytes dropped): every path under `wiki/` is longer than 3 bytes and the status is scoped to `-- wiki`; not reachable in practice.
- **R8** Code-review skill claim at `src/agent/solve.rs:252` (`cancel` of a `--wait` run kills iter and orphans `claude`): from an interactive shell iter is its own process-group leader and `kill -TERM -- -<pid>` reaches the child too; only non-interactive launches hit the fallback. Kept out of the table as Low/situational.

## Confidence

High on C1, C2, C3, C5, C6, C8 (reproduced). High on C4, C9, C10, C11, C13, C14, C15 (precise read of short code paths). Medium on C7 (reasoned from naive-time arithmetic, not observed at a clock change) and C16 (depends on agent output). Severity for C3 is debatable: the behaviour is documented, the finding is the missing guard.

## Out-of-scope observations

Security (for the security lens):
- **O1 (run)** `Organization::check` (`src/models/organization.rs:173-178`) accepts `http://localhost:1@evil.example/api`: `iter org new` with that `platform_url` saved it (`sqlite3 ... select platform_url` shows it). The host is `evil.example`, and the bearer key goes there over plain HTTP. The code-review skill flagged the same. Parse with a URL parser and compare the host.
- **O2 (read)** `src/agent/run.rs:207-212` and `src/agent/solve.rs:295-301`: the pid used by `cancel` is read from `runs/<n>.yaml`, which the agent can write (`--add-dir <taskdir>`, `acceptEdits`); `pid: 0` becomes `kill -TERM -- -0`, iter's own process group.
- **O3 (read)** `src/process.rs:476-481` puts the full argv (including `gh --body <text>`) into error strings that reach the push ledger (`src/push.rs:398`).
- **O4 (read)** `src/md_edit.rs:65-70,109-114`: editor buffers in `temp_dir()` with a predictable name (`iter-edit-<pid>-<timestamp>.md`); fine on macOS per-user `TMPDIR`, weak on a shared `/tmp`.

Web (from the code-review skill, not verified by me): `src/web/task_runs.rs:375` rejects a queued run's old model after `agent.models` changes; `src/web/forms/project.rs:46` turns a stale organization id into a 500; `src/web/platform_pick.rs:51` makes two synchronous HTTP calls per render; `src/web/search_index.rs:159` rebuilds the index per keystroke.

Performance seen in passing: `RunRecord::is_running` spawns `kill` and `ps` per run and is called in loops (`src/agent/solve.rs:116,202,247`, `src/commands/task.rs:271`); `ingest_job::auto` builds the whole wiki graph before reading `mode`.

Design notes: deleting a project cascades to its tasks and sessions (`src/db/migrate.rs:106,127`), so `iter project delete` erases the time records; `task done` with the agent still running is refused (`src/commands/task.rs:270-278`), good.

## What I could not assess

- Behaviour against a real time platform and `gh` (C10, C11, C13 are read-only), and the agent harness (`claude`) end to end.
- Concurrency between the running web server and the CLI on the same store beyond reading the code (C4, C12).
- An actual DST transition (C7).
- The `wiki/mv.rs:93` BOM claim from the code-review skill (plausible from `frontmatter::split` stripping a BOM where `rewrite` does not; not verified by run).
