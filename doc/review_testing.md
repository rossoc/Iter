# Review: testing (src/web focus)

## Scope
All tests in the crate (inline `#[cfg(test)]` only; there is no `tests/` directory), with focus on `src/web/**`. Branch `feat/html-refactor` @146aeae. `doc/web_*.md` not read. `doc/review_*.md` not read. No `src` edited.

## Method
Done:
- Verified by running: `cargo test` in the worktree (fails to compile, see F1). `cargo test` in a scratch copy with a one-line import added to `agenda_pick.rs` (302 passed, 0 failed, 0.56s). `cargo test -- --list` for the inventory.
- Grep counts of `#[test]` per file. Read the tests and code of: guard, drop, errors, edit, agenda_pick, ink, board_cards, url, quadrants, task_card.
- Everything else is inferred from names and light reads.

Not done:
- No coverage tooling run (none checked as installed; I did not install any).
- No browser, no HTTP-level run of the server.
- JS (`board.js`, `org.js`) not executed.
- Not every test body read.

## Test inventory
- 302 tests in total, in 57 files. `db.rs` has 57, `reporting.rs` 32, `sync.rs` 20, and 117 are in `web::`.
- The web tests are all pure-function or unit tests: URL building, query parsing, form decoding and validation, ordering, flag mapping, and a few DB-backed ones (`forms::task::create_writes...`, `load::*`, `settings::*`).
- Of ~113 files in `src/web`, about 61 have no `cfg(test)` block. Most are view-only (`ui/*`: 35+ files) or route glue (`home`, `org`, `project`, `task`, `boards`, `task_edit`, `org_edit`, `project_edit`, `board_edit`, `board_info`, `settings_panel`, `org_report`, `edit`, `layout`).
- There are 0 tests that render a view or HTML. A grep for render/html in tests found none.
- There are 0 tests that make an HTTP request through `router()`. There are 0 JS tests.

## Findings

### Broken tests
**F1 (Critical, verified by running). `cargo test` does not compile.** `src/web/agenda_pick.rs:96-101` calls `with_side`, but the test module imports only `super::*` and `models` (lines 52-53). The compiler says "cannot find function `with_side`" (3 errors). The whole suite, 302 tests, is therefore unrunnable on this branch. Fix: add `use crate::web::url::with_side;` to the test module (this alone made all 302 pass in my scratch copy). Check that `agenda_pick.rs` no longer needs it at the non-test level.

### Redundant or overlapping
**R1 (Low, inferred from reading).** `board_cards.rs:89` `only_a_hex_color_reaches_the_style` and `ui/ink.rs:83` `only_a_hex_color_reaches_a_style` both check hex-color filtering. The ink test is exhaustive (8 bad inputs). `Colors::style` only calls `is_hex_color`. Keep the `style()` test for the joining logic (`;` separator) and drop the bad-input cases, or point both at the shared `is_hex_color` test in `models/tag.rs`.

**R2 (Low, inferred).** `agenda_day::a_board_without_cards_is_empty` and `quadrants::a_board_without_cards_is_empty` share the same shape. `agenda_day` and `quadrants` also share list-ordering tests ("urgent first, in progress first"). They test different modules, so keep them, but a shared fixture builder would remove the duplicated setup. I did not diff the setups.

**R3 (Low).** `quadrants::every_priority_has_its_quadrant:142` and `task_card::every_priority_has_its_edge:134` each map the four priority combinations by hand. This is a deliberate per-type mapping, not a real duplicate. The same is true of the `matrix` and `agenda_pick` "moved vs. scheduled" word tests, which use the same `PickWords::of` (`pick.rs` already has `a_move_and_a_pick_are_worded_with_the_task`).

No large-scale duplication found. Tests are small, named by behavior, and mostly one concern each.

### Missing tests
**M1 (High). No test of the HTTP surface.** `router()` (`web/mod.rs:82`), the three layers (`blocking`, `guard::local_only`, `errors::error_pages`) and every route handler are untested end to end. Proposed: a test-only harness that builds the router against a temp DB (`open_db` reads `config().db_path()`, so this needs a config override) and asserts:
- GET `/` returns 200.
- GET `/org/999` returns 404 HTML (the `errors.rs` document).
- GET `/org/abc` returns 400.
- A POST with a foreign `Origin` returns 403.
- A POST failure is not rewritten into a page (`errors.rs:30`, `method` GET/HEAD only).
The `errors.rs` page text, the status codes and the "GET only" rule have zero coverage today.

**M2 (High). No XSS or escaping tests.** User-controlled strings (task title, tags, org/project names, notes, free text in a search) go into views. No test renders a view with `<script>`, `"` or `&`. Only `errors.rs` documents "static text only, so nothing needs escaping". Proposed: render the task card, task table, crumbs, footer, org list and form fields with `name = "<img src=x onerror=1>\"&"` and assert that the output contains `&lt;img` and no raw `<img`. If the view library escapes by construction, one parametrized test over a few key views is enough to lock that in. Also test `style=` injection: `is_hex_color` is tested, but the end-to-end `style` attribute for chips (`task_rows::chips_carry_the_tags_name_and_color`) is only checked as a string.

**M3 (High). `guard.rs` edge cases.** Existing tests (`guard.rs:58-73`) cover the happy path and one foreign origin. Missing:
- Host with no port: `"localhost"`, `"127.0.0.1"`. This works because `host_of` is `rsplit_once(':')`, but it is untested.
- IPv6 `[::1]:3000`, which is documented as unsupported and is rejected. Pin that with a test.
- Look-alike hosts such as `127.0.0.1.evil.com` and `localhost.evil.com`. They are rejected today, but there is no test.
- `Origin: null`, which is sent by sandboxed iframes and `file://`. It currently fails the `split_once("://")` check, so it is rejected. Pin that.
- A scheme mismatch (`https://127.0.0.1:3000` against host `127.0.0.1:3000`) is accepted, because the code compares `rest` only.
- HEAD and OPTIONS count as non-GET (`is_get` is computed in `local_only`, not in `allowed`).
- A missing `Origin` on a POST is allowed (tested at line 70). A DNS-rebinding attempt is blocked only by the `Host` check.

**M4 (High). `drop.rs::drop_on` and `agenda_drop::schedule_task` error paths are untested.** `drop.rs:35-50` has three distinct 404 paths (task missing, project missing, task on another board) and a 400 path (`apply` returns Err). `drop.rs` tests only `back_or`. `pick::only_a_card_of_the_board_can_be_picked` covers part of the board check on the pick side. Proposed: DB-backed tests for each of the 404s, the 400, and that `update_placed` is atomic (flags and placement are written together).

**M5 (Medium). `edit::settle` / `submit` / `create_to` (`edit.rs:114-175`) have no tests.** This is the shared save-or-refuse path for all forms. Proposed: with a fake `EditForm`, assert that
- a valid form returns `Err(see_other(land(id)))`,
- an invalid form returns `Ok(Refused)` with what the user typed kept (`row`, `stored`, `error`),
- `extra` is only called on a refusal.
The task and project forms test their own `apply`, but nobody tests the shared flow.

**M6 (Medium). Accessibility and HTML structure.** Only `ui/field.rs` has a11y-oriented tests (ids from name, `aria-describedby`, checklist numbering). Also `ink::every_color_reads_at_4_5_to_1` checks contrast. Nothing asserts: the skip link and `main id` on the page frame (`frame.rs` tests only the title), `<html lang>`, the heading hierarchy (`h1` once per page), `aria-current` on the tabs (`board_header::a_board_has_the_three_tabs` and `sections::section_tabs_mark_the_current_one` check the model, not the markup), labels for every input, `aria-live` on notices, form error summary linking to the field. Proposed: a render helper and per-page structure assertions: exactly one `h1`, `main#main` exists, every `input` has an associated `label`, and `aria-current="page"` appears once in the tabs.

**M7 (Medium). Unicode, empty and large input.** None of the web tests use non-ASCII text, apart from the sort test in `load`. Gaps:
- `task_query`: the quoted-word splitter has no unclosed-quote or unicode case (`"é` and a title with emoji). Unicode case-insensitive tag matching (`É` vs `é`) is untested.
- `folders::normalize`: the `..` test exists. Missing: a path with a trailing `/`, a symlink, a non-UTF-8 name, and a non-existent typed path.
- `url::page_url`: tested with `a b&c`. Missing: unicode, `#`, `?`, `%`, `+`.
- `forms::task`: very long titles, an empty title (checked as a refusal), and a title that is only whitespace.
- `notes::the_first_line_with_text_is_taken`: check CRLF and a note with no text.
- Large lists: `load`, `agenda_day` and `sessions_table` are not tested at scale. Low priority for a local tool.

**M8 (Medium). Date and time edges.** `agenda::a_bad_or_missing_date_is_today` is good. Missing: a DST boundary day in `agenda_day` (23/25 hours), midnight and end-of-day sessions in `session_lines`, tasks that span midnight, and `the_now_line_is_only_in_the_current_hour_of_today` at 23:59 versus 00:00. `utils/clock.rs` has 3 tests; I did not check whether `now()` in `web/mod.rs:78` is injectable (it looks hard-wired to `Local::now()`, so the pages that call it cannot be tested with a fixed time except through the pure helpers).

**M9 (Medium). `assets.rs` ETag handling.** Only `a_tag_in_the_list_matches` exists. Missing: `respond` returns 304 on a match, 200 plus `ETag` and `Content-Type` otherwise, and `If-None-Match: *` and weak `W/"..."` tags (`matches` at `assets.rs:76`).

**M10 (Low). JS (`board.js`, 141 lines; `org.js`, 58 lines)** drop and pick behavior is untested. A server-side contract exists (`drop::back_or`, JSON `true`), but the client half does not. If the project wants to keep JS, add a Node-based smoke test or at least a test that every `data-` attribute the JS reads is emitted by the Rust views.

**M11 (Low). Untested route modules** `org_report.rs` (the report tab, date range parse), `settings_panel`, `board_info`, `board_edit`, `home`, `boards`. Their logic is thin, but `org_report` has 11 functions and no test. Check whether bad `from`/`to` values are refused or ignored.

**M12 (Low). Folder browser safety** (`folders.rs`). It lists any directory the process can read. `dot_segments_do_not_climb_past_the_root` exists, but there is no test for a hidden-folder toggle, symlink loops, an unreadable directory (`subfolders` returns `Option`, `None` path untested), or a path that is a file.

## Confidence
- High: F1 (reproduced), the 302-pass result with the import fix, the inventory counts, the guard and drop gaps (code read).
- Medium: the redundancy findings (names plus partial reads).
- Medium-low: M2, M6 and M7. They depend on how the view layer (`topcoat`) escapes and renders, which I did not verify. They may be partly guaranteed by the library, but there is still no test that locks it in.

## What I could not assess
- Real line or branch coverage (no tool run).
- Whether `topcoat` offers a test client or render-to-string API for M1, M2 and M6.
- Runtime behavior of the JS, and rendered a11y (focus order, screen reader output).
- Flakiness: one run only, 0.56s, no shared-state or temp-dir collisions seen, but `folders` tests use a temp tree and `config()` is global, so parallel safety is unchecked.
- Tests in non-web modules (`db.rs`, `reporting.rs`, `sync.rs`) beyond counting.
