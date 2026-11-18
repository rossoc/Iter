# Correctness review of `src/web/` (2026-11-02, lens `correctness_web`)

## Summary

The web layer is in good shape: every `?tab=`, `?q=`, `?date=`, `from`/`to` and path edge case I threw at the shared server answered 200/400/404 without a 500 or a panic, all 543 unit tests pass, `cargo test` is clean, dates are `Naive*` throughout so DST cannot bite, the ETag/304 path works for strong and weak tags, HEAD is served, and the CSRF guard does what its comment says. The defects are concentrated in three places: the Share pop-up's push (a confirmed POST is re-executable and the `already_pushed` check is computed and discarded), the Agent compose form (a task-folder instruction is offered and previewed but refused on submit), and the write side of forms (a stale edit form silently undoes what the boards changed; a few text fields keep CRLF).

Top 5:

1. **CW-1 (High, run)** `org_share.rs:298-301` + `push.rs::execute`: re-POSTing the confirm form (what a browser refresh of the 200 result page does) pushes the same worklog a second time when the platform's `GET /time-entries` does not list the first push; `Check.already_pushed` is true and ignored. Two rows with the same hash landed in `pushes`, two POSTs hit the fake platform.
2. **CW-2 (High, run)** `task_runs.rs:410,473` validate the instruction against `layers_of(db, project)` without the `Source::Task` layer that `task_agent.rs:110-113` and `prompt_of` (`task_runs.rs:433`) add: an instruction in `tasks/<id>/instructions/` previews fine (`GET /task/1/prompt?instruction=review` → 200) and is refused on `POST /task/1/runs` with `400 'review' is not an instruction`.
3. **CW-3 (Medium, run)** `forms/task.rs:103-122` + `edit.rs:130`: the task edit form writes `start_time`, `duration` and the Urgent/Important tags from what was typed, so saving a form opened before a board drag undoes the drag: a task scheduled at 09:00 on the agenda went back to unscheduled and lost both matrix flags when a stale `/task/7/edit` was saved.
4. **CW-4 (Medium, run)** `task_runs.rs:417,474`, `solve.rs:140-141,193`: the agent prompt textarea is stored verbatim with CRLF (`runs/1.prompt.md` = `line1\r\nline2\r\n`), unlike every other textarea (`forms::text`); `task_sessions.rs:66` keeps CRLF inside the session note too.
5. **CW-5 (Medium, run, re-confirmed from `doc/web_final_review.md` A6b)** five pop-up POST routes answer a bare `text/plain` 400 and the typed form is gone: `task_notes.rs:94`, `task_sessions.rs:81`, `instructions/save.rs:66-90`, `task_new.rs:177,217`, `task_runs.rs:410,473`.

Counts: Blocker 0 · High 2 · Medium 5 · Low 9.

## Scope

`src/web/**` (all `.rs`, `.js`, `.css` referenced by behaviour), read in full: `mod.rs`, `blocking.rs`, `errors.rs`, `guard.rs`, `assets.rs`, `load.rs`, `edit.rs`, `forms/*`, `task_runs.rs`, `task_agent.rs` (to line 140), `drop.rs`, `agenda_drop.rs`, `matrix.rs`, `agenda.rs`, `agenda_day.rs`, `agenda_pick.rs`, `pick.rs`, `quadrants.rs`, `board_page.rs`, `boards.rs`, `board_info.rs`, `sections.rs`, `task_query.rs`, `org_report.rs`, `org_share.rs`, `org.rs`, `project.rs`, `home.rs`, `task.rs`, `task_edit.rs`, `org_edit.rs`, `project_edit.rs`, `board_edit.rs`, `project_new.rs`, `task_new.rs`, `task_notes.rs`, `task_sessions.rs`, `task_wiki.rs`, `daily_notes.rs`, `notes.rs`, `agents.rs`, `instructions/{mod,page,list,save}.rs`, `platform_pick.rs`, `project_rows.rs`, `project_options.rs`, `task_rows.rs`, `task_fields.rs`, `settings*.rs`, `search.rs`, `search_index.rs`, `folders.rs`, `folder_picker.rs`, `footer.rs`, `layout.rs`, `url.rs`, `crumbs.rs`, `session_lines.rs`, `ui/{field,modal,form,notice,filter_bar,tasks_tab,tabs,day_nav}.rs`, `board.js`, `org.js`, `platform.js`, `ui/{modal,search,combobox,filter_bar,path_pick}.js`. Supporting non-web code read where the web code's correctness depends on it: `agent/solve.rs` (create/amend), `agent/instruction.rs` (compose, find, layers_of, file), `agent/taskdir.rs`, `push.rs` (check/execute), `platform.rs`, `db/mod.rs` (range_bounds, save_task, replace_task_tags, set_priority, update_placed), `notes.rs` (dirs), `utils/report.rs::parse_day`.

CSS was not reviewed for correctness (no behaviour). `ui/prose.rs` (markdown) was left to the security lens.

## Method

- `Skill code-review medium src/web/ --max-findings all` ran first (it reviewed the directory as a whole since the branch has no diff against `main`); its 17 candidates were treated as hypotheses and confirmed, refuted or downgraded below (CW-2, CW-11, CW-12, CW-13, CW-14, CW-15 come from it; its performance/DRY items were left to those lenses).
- Manual read of every file above, against the hunt list in the brief.
- `run` evidence against the shared read-only server `http://127.0.0.1:3999` (GETs only) and against my own scale-1 store on port 4050 (`doc/review_tools/seed.sh $SCRATCH/cw-home 1 $BIN`; `XDG_CONFIG_HOME=$SCRATCH/cw-home ITER_PLATFORM_KEY=dummy $BIN serve -p 4050 --no-open`), plus a fake time platform `doc/review_tools/correctness_web/fake_platform.py` on port 4051 (GET `/time-entries` → `{"entries":[],"has_more":false}`, POST → `{"results":[]}`, every POST body appended to a log). `org-1.platform_url` of my store was pointed at it with sqlite3. Both servers were stopped afterwards; the shared one was never written to.
- `XDG_CONFIG_HOME=$SCRATCH/test-home cargo test 2>&1 | tail -30`, verbatim tail:

```
test wiki::ontology::tests::relation_keys_are_the_authorable_properties ... ok
test wiki::mv::tests::moving_a_page_rewrites_what_points_at_it ... ok
test wiki::ontology::tests::the_rules_are_read_from_the_owl ... ok
test wiki::ontology::tests::the_built_in_ontology_says_where_classes_live ... ok
test wiki::retrieve::tests::find_stops_at_the_budget_but_keeps_the_best ... ok
test wiki::retrieve::tests::find_ranks_by_tag_path_and_description ... ok
test wiki::retrieve::tests::ids_are_bare_unless_the_page_is_in_a_level_the_question_cannot_see_into ... ok
test wiki::schema::tests::an_extension_shows_up ... ok
test wiki::retrieve::tests::grep_finds_lines_in_pages_and_in_notes_on_request ... ok
test wiki::retrieve::tests::show_goes_deeper_with_the_depth ... ok
test wiki::retrieve::tests::neighbors_walk_the_graph_nearest_first ... ok
test wiki::schema::tests::the_tables_come_from_the_ontology ... ok
test wiki::scope::tests::a_project_without_an_organization_is_its_own_scope ... ok
test wiki::scope::tests::an_organization_goes_down_into_its_projects_only_when_asked ... ok
test wiki::scope::tests::nowhere_to_keep_notes_is_an_error ... ok
test wiki::scope::tests::a_task_sees_itself_its_project_and_its_organization ... ok
test wiki::ingest_job::tests::only_one_agent_holds_the_wikis_lock ... ok
test wiki::ingest_job::tests::pending_pages_are_found_with_their_diffs_in_a_repository_above_the_notes ... ok
test wiki::ingest_job::tests::accepting_commits_the_pages_only ... ok
test wiki::ingest_job::tests::throwing_away_touches_the_pages_and_nothing_else ... ok

test result: ok. 543 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 1.53s
```
(exit code 0; one test binary, no doc-tests reported.)

What was NOT checked: JavaScript behaviour in a real browser (the `.js` findings are `read`); `osascript`/`zenity` folder picker (`folders.rs` was exercised only by reading); starting an agent (`Run now`) was deliberately never POSTed; concurrent writers racing on the same row (the lost update in CW-3 was reproduced sequentially, which is the realistic shape); Firefox/Safari form-encoding differences.

## Findings

| ID | Severity | file:line | Evidence | Effort | One-line fix |
|---|---|---|---|---|---|
| CW-1 | High | `src/web/org_share.rs:298-301`, `src/push.rs` (`execute`, the `check.already_pushed` field is never read) | run | S | In `push_step`, when `check.already_pushed` return `Outcome::Preview { already: true }` (or an Error) instead of calling `execute`; and answer a confirmed push with a 303 to the report (`?pushed=<hash>`) so a refresh cannot re-POST |
| CW-2 | High | `src/web/task_runs.rs:410,473` vs `src/web/task_agent.rs:110-113`, `src/web/task_runs.rs:432-435` | run | S | Build the layers once (`layers_of` + `Source::Task` from `taskdir::of`) in `world()` and use them in `create_run`, `edit_run` and `prompt_of` |
| CW-3 | Medium | `src/web/forms/task.rs:103-122,130-145`, `src/web/edit.rs:130-132` | run | M | Carry a `loaded_at`/row version in the edit form and refuse a save over a newer row, or make the board-owned fields (start, duration, Urgent/Important) read-only on the edit page and only write them when changed |
| CW-4 | Medium | `src/web/task_runs.rs:417,474`, `src/agent/solve.rs:140-141,193`; `src/web/task_sessions.rs:66` | run | S | Pass `&forms::text(&form.prompt)` / `text(&form.message)` like every other textarea |
| CW-5 | Medium | `src/web/task_notes.rs:94`, `src/web/task_sessions.rs:81`, `src/web/instructions/save.rs:66-90`, `src/web/task_new.rs:177,217`, `src/web/task_runs.rs:410,473`, `src/web/errors.rs:27-29` | run | M | Re-render the pop-up with `FormError` (as `edit.rs::settle` does) or at least let `errors::error_pages` dress POST 400s |
| CW-6 | Medium | `src/web/org_share.rs:319-335` | run | S | Redirect after a successful push/copy (303) instead of rendering the 200 page: a refresh re-POSTs; with a compliant platform the second POST shows "Pushed, but some steps failed." with every day marked conflict |
| CW-7 | Medium | `src/web/ui/modal.js:34-44` | read | S | On a failed prompt fetch, show the error and set `chosen.value = ""` (or re-validate on submit) instead of leaving the previous instruction in the hidden field |
| CW-8 | Low | `src/db/mod.rs:85-88` (surfaced by `org_report.rs:47-60`) | run | S | Clamp `to` to 9999-12-30 or use `checked_add` with a `NaiveDate::from_ymd(9999,12,31)` guard: `to=9999-12-31` yields `+10000-01-01` which text-compares below every row, so the report is empty |
| CW-9 | Low | `src/web/task_runs.rs:379-381` vs `:371` | run | S | `trim()` effort and instruction as the model is trimmed; `effort=medium ` is refused with 400 |
| CW-10 | Low | `src/agent/solve.rs:185-186` via `src/web/task_runs.rs:474` | run | S | A cancelled run amended from a stale Settings tab says "run 1 has begun: it cannot be edited"; say "was cancelled" |
| CW-11 | Low | `src/web/task_runs.rs:102-104` | read | S | Look `?run=N` up before `take(RUNS)`: a queued run older than the 50 newest cannot be opened from the Agent tab while `/agent/{id}/{n}` still offers Settings |
| CW-12 | Low | `src/web/instructions/mod.rs:133` | read | S | Propagate the `db.list::<Task>()` error instead of `unwrap_or_default()`: a locked/corrupt DB renders `/agents` as "No agent has worked on any task yet." |
| CW-13 | Low | `src/web/daily_notes.rs:251`, `src/web/task_notes.rs:93`, `src/web/task_sessions.rs:82`, `src/web/instructions/save.rs:91-92,169-170,183-187` | read | S | Route these `?` through `errors::refused` so a user-fixable error is a 400 with a message, not a 500 |
| CW-14 | Low | `src/web/project_rows.rs:15-22` | read | S | `tilde()` must check that the path continues with `/` after `$HOME`: `/Users/carlos/app` renders as `~s/app` for `HOME=/Users/carlo` |
| CW-15 | Low | `src/web/guard.rs:18-19` | run | S | Compare `Host` case-insensitively (`Host: LOCALHOST:3999` → 403 while `Origin` two lines later uses `eq_ignore_ascii_case`) |
| CW-16 | Low | `src/web/board.js:176-196` | read | S | Guard against a second drop while a POST is in flight (`busy` flag as `path_pick.js:9-13` does); two quick drops of the same card send two POSTs and the second reload wins |

### CW-1 · High · `org_share.rs:298-301` + `push.rs::execute` · run · S

`push_step` computes `check = push::check(..)` whose `already_pushed` is true when a complete push with the same hash exists, then calls `execute` unconditionally when `confirm && hash == plan.hash`. `execute` only consults `check.conflicts` (the platform's listing) and the ledger's *incomplete* rows. If the platform's `GET /time-entries` does not return the just-pushed days (lagging listing, a webhook that only accepts POSTs, a 2xx with an empty page), the same hours go out again. The result page is a 200 render of the POST, so a browser refresh is exactly this re-POST.

Repro (own store, fake platform on 4051, org 1 pointed at it, `T=$(date +%F)`):
```
curl -s -X POST -d "from=2020-01-01&to=$T&modality=push&format=worklog" http://127.0.0.1:4050/org/1/share -o e1.html   # preview, hash in name="hash"
curl -s -X POST -d "from=2020-01-01&to=$T&modality=push&format=worklog&hash=$HASH&confirm=1" http://127.0.0.1:4050/org/1/share   # "Pushed." 12 days ok
curl -s -X POST -d "...same body..." http://127.0.0.1:4050/org/1/share                                                               # "Pushed." again, 12 days ok
wc -l pushes.log  -> 2
sqlite3 cw-home/iter/iter.db "select id, substr(hash,1,8) from pushes"  -> 1|592f0772  2|592f0772
```
With a platform whose listing is accurate the second POST is refused day by day ("conflict"), but the page then says "Pushed, but some steps failed." (read: `execute` marks every conflicting day `Item::failed`). Fix: honour `already_pushed` before `execute`, and redirect after a push (CW-6).

### CW-2 · High · `task_runs.rs:410,473` · run · S

`task_agent::load` (line 110-113) and `prompt_of` (line 432-435) add `(Source::Task, <taskdir>)` in front of `layers_of(db, project)`; `create_run` and `edit_run` call `parse_run(&form, &instruction::layers_of(&app.db, &project))` without it. A `tasks/<id>/instructions/<name>.md` is therefore shown in the compose pop-up (`?new=agent&instruction=review` opens with its prompt, the datalist offers it once a plan exists), its prompt loads over `fetch`, and the submit fails with a bare 400.

Repro (own store, task 1 of project-1/org-1, task dir `$SCRATCH/cw-home/notes/org-1/project-1/tasks/1`):
```
printf -- '---\ndescription: review the diff\nkind: check\n---\n\nReview the task {{task}} carefully.\n' > .../tasks/1/instructions/review.md
curl -s -w '\nHTTP %{http_code}\n' 'http://127.0.0.1:4050/task/1/prompt?instruction=review'
  {"model":null,"prompt":"\nReview the task project-1/task-1 carefully.\n"}  HTTP 200
curl -s -w '\nHTTP %{http_code}\n' -X POST -d 'instruction=review&agent=claude&model=sonnet&effort=medium&prompt=hello&do=queue' http://127.0.0.1:4050/task/1/runs
  bad request: 'review' is not an instruction  HTTP 400
```
Note `task_runs.rs:433`: `if let Ok(dir) = taskdir::of(..)` can never be `Err` there because `world()` already returned on that error; the task layer should simply be part of `world()`.

### CW-3 · Medium · `forms/task.rs:103-122,130-145` · run · M

`TaskForm::apply` sets `start_time` and `duration` from the form text (blank clears them) and `save` replaces the task's tags with the form's `tags` text; `edit.rs::settle` applies this onto the row as it is *now*, so every board-owned field is overwritten with what the edit page showed when it was opened. The agenda and matrix are separate surfaces the user keeps open beside the edit page, so this is a realistic lost update, not only a concurrency corner.

Repro (own store, task 7 on project-2/board 1, no start, no flags):
```
curl -s http://127.0.0.1:4050/task/7/edit                       # start_time="", duration="", tags=""
curl -s -X POST -d 'task_id=7&target=2026-11-03+09%3A00' http://127.0.0.1:4050/board/1/schedule   # true, start=2026-11-03 09:00, duration 60
curl -s -X POST -d 'task_id=7&target=both' http://127.0.0.1:4050/board/1/matrix                   # true, tags Urgent+Important, matrix_placed=1
curl -s -X POST --data-urlencode 'name=<as loaded>' -d 'description=&github_issue=&status=queue&branch_prefix=&start_time=&duration=&tags=&move_to=2' http://127.0.0.1:4050/task/7/edit  # 303
sqlite3 ... "select start_time,duration,matrix_placed from tasks where id=7; select count(*) from task_tags where task_id=7"
  ||1   and   0
```
The task is now "placed" on the matrix with neither flag (it lands in Eliminate) and unscheduled. Within `drop_on` itself (`drop.rs:29-34`) the read and the `update_placed` are not in one transaction either, but that window is milliseconds; the form is the user-visible case.

### CW-4 · Medium · `task_runs.rs:417,474`, `solve.rs:140-141,193`; `task_sessions.rs:66` · run · S

Browsers submit textareas with CRLF. `forms::text` normalises every other textarea (`forms/task.rs:124`, `task_notes.rs:72`, `daily_notes.rs:236`, `instruction::compose`), but the run prompt is written as received and the session note only `trim()`med.
```
curl -s -o /dev/null -w 'HTTP %{http_code} -> %{redirect_url}\n' -X POST --data-urlencode $'prompt=line1\r\nline2\r\n' -d 'instruction=plan&agent=claude&model=sonnet&effort=medium&do=queue' http://127.0.0.1:4050/task/1/runs
  HTTP 303 -> /agent/1/1
od -c .../tasks/1/runs/1.prompt.md    ->  l i n e 1 \r \n l i n e 2 \r \n
curl -s -X POST --data-urlencode $'message=a\r\nb' -d 'start=2026-10-30T09:00&end=2026-10-30T10:00' http://127.0.0.1:4050/task/3/sessions
sqlite3 ... "select hex(message) from sessions order by id desc limit 1"  -> 610D0A62
```
The prompt goes to the agent with `\r`; `modal.js:38` compares `promptArea.value === promptArea.dataset.loaded`, which already differs on the first autoload for a CRLF browser value, so an edited-prompt check may misfire too (read).

### CW-5 · Medium · pop-up POSTs · run · M (re-confirmed: `doc/web_final_review.md` A6b "POST keeps raw 400/404")

```
POST /task/1/notes name=&text=x               -> 400 text/plain "bad request: a note needs a name"
POST /task/1/sessions start=nope&end=...       -> 400 text/plain "bad request: 'nope' is not a date and time"
POST /instructions name=Bad+Name&prompt=x      -> 400 text/plain "bad request: 'Bad Name' is not a name: ..."
POST /task/new project=abc&...                 -> 400 text/plain "bad request: Unknown project."
POST /task/1/runs instruction=nope&...         -> 400 text/plain "bad request: 'nope' is not an instruction"
POST /board/1/schedule task_id=9999&target=    -> 404 text/plain "not found"
POST /nope                                     -> 404 text/plain "not found"
```
The user lands on a blank text page and the typed note/session/prompt is lost (Back may or may not restore it). `edit.rs::settle` and `task_new.rs::save` (for validation errors after the project id parses) already do the right thing; the five routes above should return their pop-up with `FormError`. The messages also bypass `FormError::sentence`.

### CW-6 · Medium · `org_share.rs:319-335` · run · S

`share` is a `#[page(POST ..)]` that renders the Report tab. Copy and Push results are therefore 200 pages whose refresh re-submits (see CW-1 for the consequence). `drop.rs::back_or`, `agenda_drop.rs` and every edit route use 303; the share route should too (`?shared=copy|done&hash=`), keeping the render only for the preview and errors.

### CW-7 · Medium · `ui/modal.js:34-44` · read · S

`load()` sets `asked = name` before the fetch and, in `catch`, resets `asked` to the hidden field's value but leaves the hidden `instruction` at the previous name and the visible field at the typo, with no message. The next click on Run now / Queue posts the previous instruction. The server would refuse an unknown name (CW-5 bare 400), but a *valid-but-different* previous value is accepted: user types `reveiw`, fetch 404s, user presses Queue, a `develop` run is queued. Fix: on failure, clear `chosen.value` and show the status, or have the server validate `instruction` against the visible field (post it with the form instead of binding it to the GET form).

### CW-8 · Low · `db/mod.rs:85-88` · run · S

`range_bounds` takes `to.succ_opt()` and formats it; chrono prints years above 9999 with a sign (`+10000-01-01 00:00:00`), which sorts before `0000-...` as text, so the half-open window is empty. The `None` branch only covers `NaiveDate::MAX`.
```
GET /org/1?tab=report&from=2000-01-01&to=9999-12-30  -> 36:23 · 3 projects · 18 tasks · 120 sessions
GET /org/1?tab=report&from=2000-01-01&to=9999-12-31  -> 00:00 · 0 projects · 0 tasks · 0 sessions  "No sessions in this period."
GET /org/1?tab=report&from=&to=9999-12-31            -> same empty report
```
Other extremes (`from=0001-01-01`, `from=1800-01-01`, `+262143`, `-262144`, `2026-02-30`, `0000-01-01` on the agenda) all fell back correctly.

### CW-9 · Low · `task_runs.rs:379-381` · run · S

`parse_run` trims `model` (line 371) but not `effort` or `instruction`: `POST /task/1/runs ... effort=medium ` → `400 'medium ' is not an effort`. Native `<select>` cannot produce this, but `combobox.js` replaces the select and the datalist inputs are free text.

### CW-10 · Low · `solve.rs:185-186` via `task_runs.rs:474` · run · S

After `POST /task/1/runs/1/cancel`, the Settings tab left open on `/agent/1/1` still posts; the answer is `400 run 1 has begun: it cannot be edited` although it never began. Cosmetic wording of a correct refusal.

### CW-11 · Low · `task_runs.rs:102-104` · read · S

`roster.runs = dir.runs().rev().take(RUNS)` then `find(|r| r.n == n)`: a queued run older than the 50 most recent is invisible to `?run=N`, so the compose pop-up does not open from the Agent tab while `/agent/{id}/{n}?tab=settings` still edits it.

### CW-12 · Low · `instructions/mod.rs:133` · read · S

`db.list::<Task>().unwrap_or_default()` turns a database fault into an empty, healthy-looking `/agents` and `/instructions` ("Used 0").

### CW-13 · Low · bare `?` on write paths · read · S

`daily_notes.rs:251` (`journal::create`), `task_notes.rs:93` (`taskdir::of`), `task_sessions.rs:82` (`db.insert`), `instructions/save.rs:91-92,169-170,183-187` (fs writes) propagate `IterError` directly: a user-actionable refusal (no notes folder, read-only directory) becomes a 500 with the error text in the body, where `errors::refused` would give a 400 with the message and keep machine faults off the page.

### CW-14 · Low · `project_rows.rs:15-22` · read · S

`path.starts_with(&dir)` with no separator check: `HOME=/Users/carlo`, `base_path=/Users/carlos/app` → `~s/app` on Home, Boards and the Settings foot-note.

### CW-15 · Low · `guard.rs:18-19` · run · S

`curl -H 'Host: LOCALHOST:3999' http://127.0.0.1:3999/` → 403. Host names are case-insensitive; `Origin` on line 42 is compared with `eq_ignore_ascii_case`, so the two halves of the same guard disagree.

### CW-16 · Low · `board.js:176-196` · read · S

No in-flight guard on `drop`: two drops before the first `location.replace` fires send two POSTs; both succeed and the page reloads on whichever `moved` arrives last. Harmless for data (the server decides), confusing for the status note.

### Hypotheses checked and refuted (no finding)

- `?tab=`/`?q=` parsing (`sections.rs`, `task_query.rs`): unknown tabs fall to Info, `q="unterminated`, `q=-`, `q=tag:`, `#999999999999999999999` all 200 and never error (run). A repeated query key (`?tab=agent&tab=info`, `?q=a&q=b`) is a 400 page; no form in the UI produces duplicates (filter bar, range form, day-nav and pick-instruction forms were checked: hidden `tab` plus a base action without a query).
- `agenda_day.rs` / `org_report.rs` dates: all `NaiveDate`/`NaiveDateTime`, no local-time arithmetic, so DST cannot shift an hour; `neighbours` uses `checked_sub_days` and guards `to < today` before `+1`; `?date=` garbage falls to today (run).
- `search_index.rs` / `search.rs`: empty names, `q=/`, `q=////`, `q="`, `q=%20`, `q=%FF` (400) all fine; marks are char indexes on both sides (`chars()` / `Array.from`), so non-BMP titles highlight correctly (read + run).
- `folders.rs::start_folder`: `..` segments cannot climb past `/`, relative paths fall to `$HOME` (read, tests cover it).
- `assets.rs`: strong and `W/` weak tags both 304 with empty body and the ETag/Cache-Control headers; HEAD on pages and assets is 200 with the GET's length (run).
- `blocking.rs`: `block_in_place` + `Handle::block_on` on the multi-thread runtime is the documented pattern; each request occupies a blocking thread (512 default) which the palette's per-keystroke fetches do not approach (read).
- Tags on every save (`db::save_task` → `replace_task_tags`): rows are deleted and re-inserted on every task save, but nothing reads `task_tags` rowids, so no user-visible effect beyond CW-3.
- `drop.rs` cross-board drop → 404; `target=do` → 400 "unknown quadrant"; pick-form `form=1` → 303 to `?moved=7#card-7`; `date=` back-link rebuilt from the parsed day (run).
- `org.js` share modal: the radio is named `mode` and the server reads only the submit button's `modality`; with script the other button is disabled so Enter submits the chosen way; without script both buttons show. Correct (read).
- `search.js` has no `r.ok` check but a non-JSON 400 body fails `r.json()` and lands in the catch ("Search failed."), so no finding.

## Confidence

High for CW-1 to CW-6, CW-8, CW-9, CW-10, CW-15 (all reproduced with curl against a seeded store). Medium for CW-7, CW-11 to CW-14, CW-16 (precise reads, not executed in a browser). The severity of CW-1 depends on the platform: with a listing that reflects pushes immediately it degrades to the CW-6 confusion; with any lag it duplicates booked hours, which is why it is High rather than Blocker.

## Out-of-scope observations

Security (always reported):
- `GET /task/{id}/prompt` and `GET /org/{id}/platform-activities` do work (render a prompt with git/plan reads; call the platform with the bearer key) on a GET that the guard lets through cross-site; left to the security lens, noted in the seed list.
- `prompt_of` and `create_run` accept any instruction `name` but `instruction::find` applies `valid_name` first, so `..%2F` cannot reach the filesystem (run: 404).
- `notice(focus: true)` builds an inline `<script>` from `id`; the filter on line 31-34 keeps it to `[A-Za-z0-9_-]`, fine.
- `task_notes::stem` and `folders::normalize` cannot leave their folders (tests + read).
- `failure_message`/raw error strings on pages (`task_notes.rs:71,85`, `org_share.rs:243`): `Outcome::Error(e.to_string())` puts the platform's error text (already redacted of the key by `JsonWebhook::fail`) on the page.

Other lenses:
- Performance: `/agents`, `/instructions`, `/instruction/{name}` walk every task folder and parse every run YAML per request (`instructions/mod.rs:130-155`); `/search` rebuilds the index per keystroke; the task page opens two connections and resolves `taskdir::of` four times (`task.rs:64-102`).
- Design: `/org/{id}/notes/today` is POST-only, so a typed GET is a bare 405; the Agents table's "Stop" vs the agent page's "Stop the agent" label differ (`task_runs.rs:218`, `agents.rs:158`).
- DRY: `agents.rs:244-253` duplicates `task_runs.rs:243-252`; URLs built outside `url.rs` at `task_agent.rs:285`, `task_runs.rs:227`, `sections.rs:84,172`, `task_notes.rs:33`, `task_sessions.rs:32`, `org_share.rs:187-188`, `instructions/page.rs:21` (the last one unencoded while `instruction_url` encodes).

## What I could not assess

- Browser-side behaviour (modal focus trap, combobox keyboard paths, drag-and-drop with real `dataTransfer`, `showPicker`): the JS findings are reads.
- Starting an agent from the web (`Run now`): never exercised, to keep the machine clean; `begin`'s failure recording is a read.
- The real keychain path of `connect_real` (ran with `ITER_PLATFORM_KEY=dummy`).
- `folder_picker.rs` against `osascript`/`zenity`.
- Behaviour under genuinely concurrent requests on one SQLite file (busy_timeout, WAL); only sequential reproductions were run.
