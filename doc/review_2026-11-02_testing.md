# Review 2026-11-02 — testing

Branch `feat/review` @ `5d96f44`. Reviewer lens: testing. Source untouched; scripts and proofs under
`doc/review_tools/testing/`.

## Summary

The suite is healthy and fast: **543 passed, 0 failed, 2 ignored** (545 collected) in 1.3–1.5 s, four runs
(twice parallel, once `--test-threads=1`, once `--no-default-features`: 370 passed / 2 ignored), no
flake, no order dependence, nothing written under the config home. The October compile error (F1) is
fixed. What has not moved since October is the *shape* of the coverage: 167 web tests and not one goes
through the router, renders a page or exercises the CSRF/DNS-rebinding guard as a layer; the JS
(1 043 lines, 8 files) has no tests; `edit.rs` (the shared save-or-refuse path of every form) and
`drop.rs::drop_on` have none; `interrupt`/`alive` in `agent/run.rs` have none.

The decisive library fact is now settled: **both gaps are cheap to close.** topcoat 0.9 exposes
`Router::handle(Request) -> Response` with `Request = http::Request<Body>` and `to_bytes`, and the view
layer renders to a `String`. A standalone proof (`doc/review_tools/testing/testproof/`, 4 tests) builds a
router with a verbatim copy of `guard.rs`, gets a 403 from the guard alone and a 200 whose body has
`&lt;b&gt;` for a `<b>` in user text, in-process, no socket, 0.00 s. A `node:test` + jsdom runner
(`doc/review_tools/testing/js/run.sh`, 3 tests) runs the real `filter_bar.js` in a DOM.

Top 5:
1. **T1 (High)** `src/web/guard.rs:49-58` — the `local_only` layer is never executed by a test; the pure
   `allowed` tests miss scheme (run: `Origin: https://127.0.0.1:3999` on a POST passes), IPv6, portless
   hosts, `Origin: null`, HEAD/OPTIONS. The proof shows how to test it in-process.
2. **T2 (High)** `src/web/mod.rs:86-126` — zero tests through `router()`: layers, `error_pages`
   (400/404 HTML vs text for POST), `path_param!` 400, JSON endpoints. Re-confirmed M1 of October.
3. **T4 (Medium)** `src/push.rs:79`, `src/utils/report.rs:19`, `src/agent/task_prompt.rs:240` — ~12 tests
   read the *process* config (`config()` → `$XDG_CONFIG_HOME/iter/iter.yaml`, else `~/.config/iter/`);
   an invalid real config `exit(1)`s the whole test binary (`config.rs:175-178`).
4. **T5 (Medium)** `src/web/edit.rs:83-140` and `src/web/drop.rs:23-36` — the shared form flow and the
   three 404 + one 400 paths of a drop have no test (M4/M5 re-confirmed).
5. **T8 (Medium)** 8 JS files, 0 tests; the only Rust–JS contract test is `search.rs:307`. The helpers
   are closures, so nothing can be imported without a one-line export.

## Scope

All `#[test]`s in the crate (inline modules plus `src/db/tests.rs`, `src/reporting/tests.rs`,
`src/wiki/ingest_job/tests.rs`, `src/web/instructions/tests.rs`); fakes `FakeRunner`, `github::fake`,
`platform::fake`, `secret::fake`; the 8 JS files under `src/web/`; topcoat 0.9 sources in
`~/.cargo/registry`; the running shared server on `:3999` for header probes. Focus modules as briefed:
`web/guard.rs`, `web/forms/*`, `web/edit.rs`, `push.rs`, `platform.rs`, `sync.rs`, `wiki/graph.rs`,
`agent/run.rs`, `db/migrate.rs`, `utils/clock.rs`, `reporting/`.

## Method

Commands (all from `$ROOT`, `CARGO_TARGET_DIR=$ROOT/target`, `XDG_CONFIG_HOME=$SCRATCH/test-home`,
`ITER_PLATFORM_KEY=dummy`, `TMUX=`):

| run | command | result | wall |
|---|---|---|---|
| 1 | `cargo test` | `test result: ok. 543 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 1.54s` | 2 s |
| 2 | `cargo test` (repeat) | `543 passed; 0 failed; 2 ignored … finished in 1.45s` | 2 s |
| 3 | `cargo test -- --test-threads=1` | `543 passed; 0 failed; 2 ignored … finished in 1.34s` | 1 s |
| 4 | `cargo test --no-default-features` | `370 passed; 0 failed; 2 ignored … finished in 1.20s` + one dead-code warning (T11) | 2 s |

Wall times are with a warm debug build; the test binary itself is 1.2–1.5 s. Ignored: `tmux::tests::
real_tmux_targets_exactly_and_quotes_what_the_hooks_are_told` (`tmux.rs:536`, needs tmux) and
`wiki::graph::tests::timing_of_a_big_wiki` (`graph.rs:801`). `#[ignore` greps to 3 because `tmux.rs:533`
is a doc comment. `$SCRATCH/test-home` was empty after all four runs (`find`): no test creates a file
under the config home.

Other tooling: `doc/review_tools/testing/inventory.sh` (tests per file); `curl` header probes against
`http://127.0.0.1:3999`; the proof project `doc/review_tools/testing/testproof/` (copy of the one run in
`$SCRATCH/testproof`, `CARGO_TARGET_DIR=$SCRATCH/testproof-target`, output in `OUTPUT.txt`); the JS
runner `doc/review_tools/testing/js/run.sh` (output in `js/OUTPUT.txt`). Files read in full: `web/mod.rs`,
`web/guard.rs`, `web/errors.rs`, `web/blocking.rs`, `web/edit.rs`, `db/migrate.rs:1-44`, `agent/run.rs:
196-212, 303-400`, `config.rs:113-195`, `board.js`, `org.js`, `platform.js`, `modal.js`, `path_pick.js`,
`filter_bar.js`; `combobox.js` and `search.js` outlined plus `search.js:72-140`; test names of every
module from the run log; topcoat-router `router.rs`, `origin.rs`, `layer.rs`, `body.rs`, `request.rs`,
topcoat-view `view.rs`, `buffer/handle.rs`, `hoist.rs:315`.

NOT checked: line/branch coverage (no coverage tool run); `sync.rs`, `platform.rs`, `wiki/graph.rs`
beyond their test names and the fake server setup; test bodies in `db/tests.rs` beyond the migration
ones; the real `~/.config/iter` (never read, per brief; T4 is reasoned from code).

## Inventory

`inventory.sh`: **545 `#[test]` in 216 `.rs` files; 116 files with zero tests** (two of those are
test-only helpers, `src/fixtures.rs` and `src/wiki/testutil.rs`). Web: 167 tests (`grep -c '^test web::'`
on the run log), none through HTTP, none rendering a view. JS: 8 files, 1 043 lines, 0 tests.

Top files by test count: `db/tests.rs` 60, `reporting/tests.rs` 38, `tmux.rs` 20, `sync.rs` 20,
`github.rs` 18, `push.rs` 16, `platform.rs` 14, `wiki/graph.rs` 13, `models/task.rs` 12, `notes.rs` 11,
`md_edit.rs` 11, `agent/plan.rs` 11, `agent/instruction.rs` 11, `config.rs` 10.

Zero-test files that carry logic (lines): `web/daily_notes.rs` 359, `db/mod.rs` 333, `db/migrate.rs` 304
(tested from `db/tests.rs`), `reporting/mod.rs` 302 (tested from `reporting/tests.rs`), `commands/sync.rs`
298, `commands/wiki.rs` 272, `db/tasks.rs` 258, `commands/organization.rs` 254, `process.rs` 246,
`web/org.rs` 241, `web/instructions/page.rs` 239, `commands/project.rs` 239, `commands/ingest.rs` 211,
`db/groups.rs` 206, `web/task.rs` 202, `reporting/render.rs` 195, `web/instructions/save.rs` 190,
`reporting/merge.rs` 161, `web/edit.rs` 140, `web/mod.rs` 147, `web/org_edit.rs` 138, `web/errors.rs` 58,
`web/blocking.rs` 14, and ~40 `web/ui/*` view files.

Risky modules, tests mapped to branches:

| module | tests | covered | not covered |
|---|---|---|---|
| `web/guard.rs` | 3 (`:64-94`), 11 header combos of `allowed` | loopback host with port, foreign host, no host; same-origin POST, no-Origin POST (curl), foreign Origin, other port, `sec-fetch-site` cross/same-site/none, GET with cross-site | the layer `local_only` itself (`:50`), HEAD/OPTIONS mapping (`:53`), host without port, `[::1]:3000`, `127.0.0.1.evil.com`, `localhost.evil.com`, `Origin: null`, scheme mismatch (`:41-43`, run: accepted), case (`eq_ignore_ascii_case`) |
| `web/forms/*` | 15 | decode of ticks/repeated fields, `apply` happy/refusal, field naming, CRLF (`board.rs:63`), `create_writes…` DB path | `forms/org.rs` only 1 test; whitespace-only / very long names; `taken_*` sentences only for org and task |
| `web/edit.rs` | 0 (`edit_page.rs` has 2 for headings) | — | `load`, `load_among` 404, `submit`, `create_to`, `settle` (valid → `see_other`, invalid → `Refused` keeps typed row, `extra` only on refusal) |
| `web/drop.rs` | 1 (`back_or`) | JSON vs redirect | `drop_on` `:23-36`: task 404, `apply` 400 |
| `push.rs` | 16 | plan per day, done-only GitHub work, empty period, ledger, fingerprint stable/changed, conflicts, partial reruns, platform and GitHub failures, ids | `plan` with a `pause_gap_minutes` other than the config default (reads `config()`, T4) |
| `platform.rs` | 14 | bearer JSON, paging, cap, page vs JSON vs empty answers, errors without key, debug redaction; real loopback `TcpListener` stub (`:366-400`, port 0, no collision) | — (unicode names not tried) |
| `sync.rs` | 20 | create/adopt/clash/reopen/close/body matrix | — |
| `wiki/graph.rs` | 13 (+1 ignored timing) | levels, resolution, broken front matter, ntriples, SPARQL error | non-ASCII page names |
| `agent/run.rs` | 3 | `Harness::Claude.command` (twice, T9), run numbering/listing | `alive` `:197`, `interrupt` `:207` (incl. `pid 0` → `kill -TERM -- -0`), `shown_status` "lost" `:169-172`, cancel |
| `db/migrate.rs` | 12 in `db/tests.rs` | fresh DB, up-to-date skips (`:211`), concurrent opens migrate once (`:13`), legacy renames, old tables gain columns, flags→tags, short sessions dropped | resume from an intermediate `user_version` (every old-schema test sets `user_version = 0`, `db/tests.rs:8`); steps 4–6 only ever run after baseline |
| `utils/clock.rs` | 7 | under/over a minute, stale close at last save, nothing open | all on `2026-09-01` (`:127`); no midnight-crossing session, no DST day |
| `reporting/` | 38 | merge bridges/keeps gaps, chains, ongoing, union across tasks; range inclusivity; render escaping of `|` and newline; rounding | no session whose start and end are on different days (0 of 35 `sess(` calls); `weekday_averages` only on one week |

## Findings

| ID | sev | file:line | evidence | effort | one-line fix |
|---|---|---|---|---|---|
| T1 | High | `src/web/guard.rs:49-58` | run | S | Add an in-process router test for `local_only` and the eight missing header cases; decide on scheme |
| T2 | High | `src/web/mod.rs:86-126`, `src/web/errors.rs:23-45` | run | M | A `#[cfg(test)]` harness: `router()` against a temp DB, assert `/`→200, `/org/abc`→400 HTML, `/org/999`→404 HTML, POST error stays text |
| T3 | Low | `src/web/ui/prose.rs:107` | run | S | One parametrized render test with `<img onerror>"&` through a task card and a form field to lock in `view!` escaping |
| T4 | Medium | `src/push.rs:79`, `src/utils/report.rs:19`, `src/agent/task_prompt.rs:240` | read | S | Pass the gap explicitly (as `merged_total_minutes` already does) or set `XDG_CONFIG_HOME` in a test harness |
| T5 | Medium | `src/web/edit.rs:83-140`, `src/web/drop.rs:23-36` | read | M | Fake `EditForm` tests for valid/refused/`extra`; DB-backed tests for the 404/400 of `drop_on` |
| T6 | Medium | `src/db/migrate.rs:11-24`, `src/db/tests.rs:8` | read | S | One test that sets `user_version = 3` on a schema at step 3 and asserts steps 4–6 run once and `user_version == 6` |
| T7 | Medium | `src/utils/clock.rs:127`, `src/reporting/tests.rs:12`, `src/web/mod.rs:82` | read | M | Add a 23:50→00:10 session to merge/worklog/clock tests; a DST-day agenda test; make `now()` injectable |
| T8 | Medium | `src/web/board.js:26`, `src/web/org.js:8`, `src/web/ui/search.js:72` | run | M | Export the helpers (`window.iter = {…}`) and adopt `doc/review_tools/testing/js/run.sh` |
| T9 | Low | `src/agent/run.rs:308,379`; `src/reporting/tests.rs:28-62`; `src/web/board_cards.rs:89` vs `src/web/ui/ink.rs:83`; `src/web/forms/{board,project,task}.rs` | read | S | Merge the duplicates named below |
| T10 | Low | `src/agent/run.rs:197-212` | read | S | Test `interrupt`/`alive` through `FakeRunner`; refuse `pid 0` |
| T11 | Low | `src/agent/error.rs:23,84` | run | S | `cargo test --no-default-features` warns `RunNotStoppable`/`IngestRunning` never constructed; gate or allow |
| T12 | Low | `src/web/search.rs:162-168` vs `src/web/ui/search.js:77`; `src/web/task_query.rs` | read | S | A non-ASCII title test (`İ`, emoji) for marks; unicode in `task_query` |
| T13 | Low | `src/wiki/ingest_job/tests.rs:31`, `src/web/task_agent.rs` (two `Local::now()`) | read | S | Fix the clock in those tests |
| T14 | Low | 116 zero-test files, ~40 `src/web/ui/*` | run | L | Render-to-string tests for the views users see most (frame, task card, form field) |

### T1 · testing · High · `src/web/guard.rs:49-58` · run · S
`allowed` is tested (3 tests, 11 combinations) but the layer around it never runs in a test, and the
pure tests miss cases the October review listed (M3, re-confirmed). Probes against the live server
(`http://127.0.0.1:3999`, `curl -o /dev/null -w '%{http_code}'`):
`GET / Host: evil.example` → 403; `Host: [::1]:3999` → 403; `Host: 127.0.0.1.evil.com` → 403;
`POST /task/new Origin: https://evil.example` → 403; `Origin: null` → 403; `Sec-Fetch-Site: cross-site`
→ 403; `Origin: http://localhost:3999` with `Host: 127.0.0.1:3999` → 403 (host mismatch);
**`Origin: https://127.0.0.1:3999` → 400** (passed the guard, refused by the form: the scheme is never
compared, `guard.rs:41-43`); `HEAD /` → 200; `OPTIONS /` → 405. None of these is pinned by a test. The
proof `doc/review_tools/testing/testproof/src/main.rs` (`a_foreign_host_is_forbidden_by_the_guard_alone`,
`a_cross_origin_post_is_forbidden_by_the_guard_with_the_framework_check_off`) shows the layer can be
driven in-process in ~15 lines; it also pins the scheme gap as current behaviour. Note that the 403 on a
cross-origin POST to an *unmatched* path (`POST /nope`) comes from topcoat's own `OriginPolicy`, not from
`guard.rs` (a layer at `"/"` wraps matched routes only, topcoat-router `layer.rs:135-145`; the proof's
fourth test shows `GET /nope` with `Host: evil` is a 404, not a 403). See observation O1.

### T2 · testing · High · `src/web/mod.rs:86-126` · run · M
`grep -rn '\.handle(' src/web` → 0. The three layers (`blocking::off_the_workers`, `guard::local_only`,
`errors::error_pages`), the `path_param!(id: i64, error = bad_request)` 400, the 404 page, the "GET/HEAD
only" rewrite rule (`errors.rs:27`), the JSON routes (`/search`, `/org/{id}/platform-activities`, `POST
/folders/pick`) and every `register` are untested end to end. Feasibility is proven: `Router::handle`
(topcoat-router `router.rs:73`, `pub async fn handle(&self, request: Request) -> Response`), `Request =
http::Request<Body>` (`request.rs:19`, built with `Request::builder()…body(Body::empty())`), response body
via `to_bytes(body, usize::MAX)` (`body.rs:217`); all re-exported from `topcoat::router`. The one knot is
`open_db()` → `config().db_path()`: a harness must set `XDG_CONFIG_HOME` to a temp dir before the first
`config()` call (it is a `OnceLock`, `config.rs:168`) or add a test-only override. The proof ran 4 such
tests in 0.00 s on a multi-thread runtime (`blocking.rs` needs `block_in_place`, so use
`new_multi_thread`, not `current_thread`).

### T3 · testing · Low · `src/web/ui/prose.rs:107` · run · S
Escaping is by construction: the proof's `a_get_renders_html_with_user_text_escaped` prints
`<h1>&lt;b&gt;x&lt;/b&gt; &amp; "q"</h1>` for `view! { <h1>(name)</h1> }`, and the only raw-HTML
emitters are `errors.rs:14-18` (static text) and the markdown writer, whose `<script>` neutralisation is
tested (`prose.rs:107-108`, the only `&lt;` assertions in the crate). Downgraded from October's M2 (High)
to Low: one lock-in test through a real component would still be cheap insurance against a future
`Html(format!(…))`.

### T4 · testing · Medium · `src/push.rs:79` · read · S
`plan()` calls `config().pause_gap_minutes`; `utils/report.rs:19 total_minutes` too. `config()` →
`Config::load()` → `config_file()` → `$XDG_CONFIG_HOME/iter/iter.yaml`, else `$HOME/.config/iter/iter.yaml`
(`config.rs:113-128, 192-205`). So the 10 push tests that go through `the_plan` (`push.rs:512-513`) and
`an_empty_period_is_refused` read the developer's real config when the variable is unset: a non-default
`pause_gap_minutes` changes merged minutes and therefore `the_fingerprint_is_stable` /
`an_identical_second_push_is_flagged…`; an unparsable file calls `std::process::exit(1)` from inside the
test binary (`config.rs:175-178`), killing every other test. `task_prompt.rs:240` asserts a path under
`config_dir()` (read-only, nothing created: `locate` does not `ensure()`). Run evidence that nothing is
*written*: `$SCRATCH/test-home` empty after four runs. Fix: thread the gap through `plan` (the reporting
layer already takes it explicitly, `reporting/merge.rs:134`) or make `fixtures` set `XDG_CONFIG_HOME` to a
scratch dir once.

### T5 · testing · Medium · `src/web/edit.rs:83-140`, `src/web/drop.rs:23-36` · read · M
Re-confirmed M4 and M5 of October, unchanged. `edit.rs` has no `#[cfg(test)]` at all; `settle`'s
contract (valid → `Err(see_other(land(id)))`, invalid → `Ok(Refused{row typed, stored, extra, error})`,
`extra` only on refusal) is what every edit page relies on. `drop.rs` tests `back_or` only;
`drop_on`'s `ok_or_not_found` and `map_err(bad_request)` paths are untested (the agenda twin,
`agenda_drop.rs`, has 4 tests of its own).

### T6 · testing · Medium · `src/db/migrate.rs:11-24` · read · S
Good coverage of "fresh", "fully up to date" (`db/tests.rs:211`), "concurrent" (`:13`) and six
old-schema shapes, but every old-schema test starts at `user_version = 0` (`db/tests.rs:8` and the
`CREATE TABLE` fixtures at `:553, :589, :619, :1037, :1083, :1445, :1482`). The resume loop
(`MIGRATIONS.get(done)`, `:17-22`) from a *middle* version — the real upgrade path of an installed
database — is never exercised, and steps 4–6 (`add_notes_path`, `add_platform`, `add_pushes`) only run
right after `baseline`. Idempotence of a single step re-run is also unpinned.

### T7 · testing · Medium · `src/utils/clock.rs:127`, `src/reporting/tests.rs:12` · read · M
Dates in tests: `clock.rs` fixes every time on 2026-09-01; `reporting/tests.rs` on 2026-09-04; none of the
35 `sess(…)` calls spans two dates; `agenda_day.rs:108-109` does place cards at 00:00 and 23:59 (good,
partial M8). No DST day, no leap day (2028-02-29), no session crossing midnight in merge, worklog
(`entries_are_one_per_day_with_merged_hours`) or `close_stale_session`. `models/session.rs:58` uses
2025-03-09 (a US DST date) but on `NaiveDateTime`, so it proves nothing about DST. `web/mod.rs:82 now()` is
hard-wired to `Local::now()`, so pages cannot be tested at a fixed time except through pure helpers.

### T8 · testing · Medium · `src/web/board.js`, `src/web/org.js`, `src/web/ui/*.js` · run · M
What the files do and what is unit-testable:
- `board.js` (197): date-apply (`ready`, `go`, typing/focusout rules, `:20-50`) — `ready` pure; drag/drop
  → `fetch(POST)` → reload with `?moved=`; scroll save/restore in `sessionStorage`; `fail()` notice. DOM.
- `org.js` (80): same date-apply for the report range (`ready` accepts `""`), hover `.whole` class, share
  modal mode switching + clipboard. `ready` pure; rest DOM.
- `platform.js` (30): debounced fetch of activities into a `<datalist>`. DOM + fetch stub.
- `modal.js` (77): inert footer, Escape → close link, Tab wrap, instruction autoload via fetch. DOM.
- `path_pick.js` (34): folder button → `POST /folders/pick` → fills input. DOM + fetch stub.
- `filter_bar.js` (13): Escape dismisses the tip. DOM (proved).
- `combobox.js` (358): `Popup` class, `enhanceInput`, `enhanceSelect` with ARIA sync. DOM.
- `search.js` (254): palette; `marked(title, marks)` (`:72`) and `words()` (`:84`) are pure modulo
  `document.createElement`; `KINDS` is pinned from Rust by `search.rs:307`.
Runner: `doc/review_tools/testing/js/run.sh <deps-dir>` installs `jsdom@26.1.0` outside the repo and
runs `node --test` (node v26.5.0). Output (`js/OUTPUT.txt`): 3 pass — `date_ready.test.mjs` (the two
`ready` helpers copied from `board.js:26` / `org.js:8`; it also documents that `2026-13-45` passes the
regex) and `filter_bar.dom.test.mjs` (the real `filter_bar.js` loaded into jsdom). To test the real
`ready` rather than a copy, the IIFEs must expose it (e.g. `window.iter = { ready }`).

### T9 · testing · Low · redundant tests · read · S
- `agent/run.rs:308 the_harness_dispatches_to_its_backend` and `:379
  claude_runs_headless_with_the_prompt_and_the_model` both assert `Harness::Claude.command()` argv for
  near-identical input; keep one.
- `reporting/tests.rs:28-62`: five `round_to_half_hour_*` tests for one four-branch function; one table
  test reads better.
- `board_cards.rs:89 only_a_hex_color_reaches_the_style` vs `ui/ink.rs:83 only_a_hex_color_reaches_a_style`
  (October R1, re-confirmed: both still present).
- `forms/{board,project,task}.rs *_errors_name_their_field` each re-assert the `name_field` default that
  `forms/mod.rs:144 name_errors_belong_to_the_name_field_by_default` already covers; keep only the
  form-specific fields.

### T10 · testing · Low · `src/agent/run.rs:197-212` · read · S
`alive` (`kill -0` + `ps -o stat=`) and `interrupt` (`kill -TERM -- -<pid>` then `kill -TERM -- <pid>`)
spawn real processes and have no test through `FakeRunner`. `interrupt(0)` would run `kill -TERM -- -0`,
which signals the caller's own process group (seed finding, confirmed by reading; not run). `shown_status`
"lost" (`:171`) is untested; the run fixture uses `pid: Some(42)` (`:359`) but no test reads `shown_status`.

### T11 · testing · Low · `src/agent/error.rs:23,84` · run · S
`cargo test --no-default-features` prints `warning: variants RunNotStoppable and IngestRunning are never
constructed` (constructed in `agent/solve.rs:288` and `wiki/ingest_job/spawn.rs:61`, which the CLI-only
build evidently does not compile). Harmless; a `#[cfg_attr]` or `#[allow]` keeps the CLI build clean.

### T12 · testing · Low · unicode · read · S
Non-ASCII appears in test code of only 5 files (`task_agent.rs`, `wiki/mv.rs`, `agents.rs`, `git.rs`,
`agent/prompt.rs`). `search.rs:162-168` computes `marks` as char indices of `title.to_lowercase()`
(`fuzzy`, `:112`), and `search.js:77` applies them to `Array.from(title)` of the original; they diverge
when lowercasing changes length (`İ` → `i̇`). `marks_cover_the_matched_title_chars` (`search.rs:301`) is
ASCII only. `task_query.rs` quoted-word splitting has no unicode or unclosed-quote case (M7 re-confirmed;
CRLF is now covered: `forms/board.rs:63`, `task_notes.rs:118`, `agent/instruction.rs:502`).

### T13 · testing · Low · wall clock in tests · read · S
`wiki/ingest_job/tests.rs:31` reads `Local::now().date_naive()` after the code under test took its own
`now()`; `web/task_agent.rs` builds `App{now: Local::now()}` twice. A run straddling midnight can read the
wrong day's note. Not observed (Low, latent).

### T14 · testing · Low · 116 zero-test files · run · L
Mostly view glue (`web/ui/*`, route `register`s) and CLI command modules (`commands/*`, 7 files, 0 tests,
~1 500 lines). The web half becomes testable with T2's harness or directly: `view.single().await` →
`ViewHandle::render(&Cx::default()) -> String` (topcoat-view `buffer/handle.rs:179`, the crate's own tests
use the pattern at `hoist.rs:315`).

## Prior review (`doc/review_testing.md`, 2026-10-01) — status

| item | status |
|---|---|
| F1 compile error (`agenda_pick.rs` `with_side` import) | **fixed since** (`agenda_pick.rs:43`); 543 pass |
| M1 no HTTP-surface test | re-confirmed (T2); feasibility now proven |
| M2 no XSS/escaping test | partly fixed (`prose.rs:107`); downgraded to Low (T3) |
| M3 guard edge cases | re-confirmed, test module unchanged (T1); scheme gap run-confirmed |
| M4 `drop_on` error paths | re-confirmed (T5) |
| M5 `edit::settle/submit/create_to` | re-confirmed (T5) |
| M6 a11y structure assertions | re-confirmed (0 `aria-current`/`h1`/`main` assertions) |
| M7 unicode/empty/CRLF | partly fixed (CRLF ×3); unicode open (T12) |
| M8 date edges | partly fixed (`agenda_day` 00:00/23:59); DST/midnight open (T7) |
| M9 `assets.rs` ETag 304/weak | re-confirmed (still 1 test) |
| M10 JS tests | re-confirmed (T8); runner proven |
| M11 `org_report` etc. | partly fixed (`org_report` 1 test); `settings_panel`, `board_info`, `board_edit`, `home`, `boards` still 0 |
| M12 folder browser | partly fixed (`folders.rs` now 2 tests) |
| R1 hex-color duplicate | re-confirmed (T9) |

## Confidence

High: suite results, flake/order checks, T1 (probes + proof), T2 (grep + proof), T8 (runner ran), T9,
T11. Medium: T4 (read; the config path is unambiguous, but which stored values would flip a test depends
on the developer's file), T6, T7 (read of test fixtures, not of every body). Low: T13 (latent).

## Feasibility verdicts

**HTTP tests in-process: YES.** `doc/review_tools/testing/testproof/` (`Cargo.toml` depends on
`topcoat = "0.9"`, `default-features = false`, `features = ["router","view","serve"]`, as `Cargo.toml:38`).
`cargo test -- --nocapture` with `CARGO_TARGET_DIR=$SCRATCH/testproof-target`: compiled from a cold cache
in ~1 min, `test result: ok. 4 passed; 0 failed … finished in 0.00s` (`OUTPUT.txt`). Recipe:
`Request::builder().method(m).uri(p).header(k,v).body(Body::empty())` → `router.handle(req).await` →
`to_bytes(response.into_body(), usize::MAX)`. `OriginPolicy::dangerous_disable()` isolates the app's guard
from the framework's. `view!` output is a `String` via the route response or `ViewHandle::render`.

**JS tests: YES, minimal.** `node:test` for pure helpers (no install), `jsdom` for scripts that touch the
DOM; `doc/review_tools/testing/js/run.sh` does both (3 pass). Blocker to testing the *real* helpers: every
file is an IIFE with no exports.

## Out-of-scope observations

- **O1 (security, Low):** `guard.rs:36-46` duplicates topcoat's default `OriginPolicy` (topcoat-router
  `origin.rs:125-193`), which runs before all layers (`router.rs:194`) and already denies cross-origin
  non-GET requests. Differences worth a comment in `doc/web_server.md`: topcoat trusts
  `Sec-Fetch-Site: same-origin` without comparing `Origin`; neither compares the scheme; only the guard
  checks `Host` (DNS rebinding). The guard's origin branch is therefore unobservable over HTTP unless the
  policy is disabled — which is what a test must do to cover it.
- **O2 (security):** `interrupt(0)` → `kill -TERM -- -0` (T10) is the seed's `pid: 0` concern; a run
  record is agent-writable (`runs/<n>.yaml` under `--add-dir`).
- **O3:** `search.js:32` frames own pages; nothing here changes the seed's `SAMEORIGIN` note.
- **O4:** `the_script_completes_the_same_kinds` (`search.rs:307`) is a good pattern: a Rust test pinning a
  JS constant via `include_str!`. The same could pin the `data-` attribute names `board.js` reads
  (`data-post`, `data-task`, `data-target`) against the views that emit them.

## What I could not assess

Line/branch coverage (no tool run); whether a developer's real `~/.config/iter/iter.yaml` currently
changes any result (never read, per brief; T4 is from code); runtime behaviour of `combobox.js` and
`search.js` beyond the Escape proof; the bodies of `sync.rs`, `platform.rs`, `wiki/graph.rs` tests beyond
names and fixtures; rendered accessibility (M6) — left to the accessibility lens.
