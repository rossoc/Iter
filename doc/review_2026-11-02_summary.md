# Review 2026-11-02 — reconciliation summary

Branch `feat/review` @ `5d96f44`. Inputs: the ten lens reports (`doc/review_2026-11-02_{security,correctness_core,correctness_web,performance,simplification,composability,design_consistency,accessibility,testing,build}.md`), `doc/review_tools/browser_pass_notes.md`, `doc/review_tools/SEED_FINDINGS.md`. Severities below are re-rated on the brief's rubric; "cosmetic maintenance = Low however structural". Evidence = best across lenses (run > read > inferred). Source IDs: SEC, C (core), CW, P, SIM, CO (composability `C1-C16`, prefixed here to avoid the clash with core `C1-C19`), DC, A11Y, T, F (build), BP (browser pass).

## 1. Headline

The codebase is sound where it counts: the CSRF/rebinding guard behaves as documented, SQL is constants-plus-placeholders, every DB query is indexed, 543 tests pass with no flake, the clone rate is 0.4 %, the web UI is one design with a complete dark theme, labelled landmarks, a keyboard path for every drag action, and no byte-slicing or DST bug in the web layer. What is wrong sits in the layer *around* the good core: the server sends no security headers (any web page can frame it and have the user click "Run now", which launches `claude --permission-mode acceptEdits`); the platform-URL check parses with `starts_with` (the bearer key goes in clear to `http://localhost:1@evil.com`); the agent can rewrite its own run record and the Stop button then trusts the pid; a confirmed worklog push is re-executable on refresh; the top bar stopped fitting 320 px when two nav items were added on 2026-10-31; four pop-up POSTs still answer a bare `text/plain` 400. Structure-wise there are seven module cycles and ~300 removable lines, none user-visible. Build-wise the `Iter` crate itself is 52 % of a clean build and a `dist` profile would halve the binary without touching the dev loop.

**Distinct findings after dedup and re-rating: 133 — Blocker 2 · High 14 · Medium 45 · Low 72** (source reports listed ~150 IDs; 17 merged across lenses).

**Fix first (all small except the third):**
1. **S-01** add `X-Frame-Options: SAMEORIGIN` + `Content-Security-Policy: frame-ancestors 'self'` (and `nosniff`, `Referrer-Policy`) in one layer next to `guard::local_only`; refuse non-GET with `Sec-Fetch-Dest: iframe|frame|embed|object`.
2. **S-02** parse `platform_url` with `http::Uri`, refuse userinfo, compare `host()`; re-check in `platform_pick::open`/`JsonWebhook::new`, not only on save.
3. **S-03 + P-01** stop trusting `runs/<n>.yaml` for `pid`/`worktree` (move records out of the `--add-dir` tree or validate `pid > 1 && getpgid(pid)==pid && comm ∈ {iter,claude}`), and probe all pids with one `ps` per render instead of `kill`+`ps` per record (1.7 s `/agents` at scale 16).
4. **C-02** honour `check.already_pushed` before `push::execute` and answer a confirmed push with a 303, so a refresh cannot double-book hours.
5. **W-02 + W-03** let `.bar` wrap under 600 px and keep `.path-cell` shrinkable: every page is 654 px wide at 320/360 px, the projects table 1119 px (WCAG 1.4.10, regression).

Next in line: **W-04** (bare 400 on note/session/run/instruction POSTs, three lenses), **C-03** (task-folder instruction previewed but refused on submit), **C-01** (`iter clone` into itself recurses until `ENAMETOOLONG`).

## 2. Deduplicated findings

Columns: ID · title · lenses · severity · evidence · effort · fix · source IDs. Lenses: SEC, CC (core), CW, PERF, SIM, CO, DC, A11Y, TEST, BUILD, BP (browser pass).

### Security

| ID | Title | Lenses | Sev | Ev | Eff | One-line fix | Sources |
|---|---|---|---|---|---|---|---|
| S-01 | No frame/baseline security headers: every one-click POST (Run now, Stop, Accept, plan steps, folder pick) is clickjackable from any site | SEC, BP, A11Y, TEST | **Blocker** | run | S | Layer sending `X-Frame-Options: SAMEORIGIN`, `CSP: frame-ancestors 'self'; default-src 'self'; img-src 'self'`, `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`; guard refuses non-GET with `Sec-Fetch-Dest` frame-ish; inline focus script in `notice.rs:43` gets a nonce or moves to `modal.js` | SEC-01, SEC-09, BP, A11Y obs, T-O3 |
| S-02 | `Organization::check` accepts `http://localhost:1@<host>` (userinfo); stored rows never re-checked | SEC, CC, SIM | **High** | run | S | Parse with `http::Uri`, reject userinfo, compare `host()` ∈ loopback; run the same check in `platform_pick::open` and `JsonWebhook::new`; tests for `@evil.com`, `127.0.0.1.evil.com` | SEC-02, CC-O1, SIM obs |
| S-03 | Agent-writable `runs/<n>.yaml` drives `kill -TERM -- -<pid>`: `pid: 0` kills the server's group, any pid kills what the agent chose; `worktree`/`status` trusted too | SEC, CC, PERF, TEST | **High** | run | M | Keep run records outside the `--add-dir` tree (or in SQLite); before signalling require `pid > 1`, `getpgid(pid)==pid`, `ps -o comm=` ∈ {iter, claude}; never fall back to a bare pid | SEC-04, CC-O2, P obs, T10, T-O2 |
| S-04 | `GET /org/{id}/platform-activities` makes an authenticated outbound call on a cross-site GET (blind; trigger for S-02) | SEC, CW | Medium | run | S | Make it a POST (only caller `platform.js:14`) or require `Sec-Fetch-Mode: cors` + `Sec-Fetch-Site: same-origin` on JSON routes | SEC-03, CW obs |
| S-05 | `ITER_PLATFORM_KEY` inherited by `claude`, `gh`, `git`, `tmux`, the editor | SEC | Medium | read | S | `env_remove(secret::ENV_KEY)` on every spawned `Command`, or read once in `main` and `remove_var` | SEC-06 |
| S-06 | Full prompt on `claude`'s argv (`ps`-visible), prompt/log/notes files 0644 | SEC | Medium | read | S | Pipe the prompt on stdin (`exec.rs` sets `Stdio::null()` today); `OpenOptions::mode(0o600)` | SEC-05 |
| S-07 | Markdown keeps `//host` links and remote images from issue bodies: referrer leaks the local port, trusted UI links out | SEC | Medium | run | S | Treat `//` as a scheme in `safe_url`; drop non-relative images or add `referrerpolicy="no-referrer"`; `img-src 'self'` via S-01 | SEC-08 |
| S-08 | tmux hooks pass session names positionally; a name starting with `-` is a clap flag (exit 2), so that task's clock never starts/stops | SEC, CC, CO | Medium | run | S | Emit `internal hook {event} -- …` in `hook_action`; `allow_hyphen_values` on the `HookEvent` positionals; strip leading `-` in `tmux_name_part` | SEC-07, C2, CO-C4 |
| S-09 | `iter.db`, run logs, prompts, notes, wiki lock created with the umask (world-readable on shared hosts) | SEC | Low | run | S | `mode(0o600)` at creation sites; `0700` on the config dir in `Db::open` | SEC-10 |
| S-10 | Editor buffers in `temp_dir()` with predictable name, follow symlinks (Linux shared `/tmp`) | SEC, CC, SIM | Low | read | S | `create_new(true)` + `mode(0o600)` or `tempfile`; one `temp_buffer()` helper (SIM-04) | SEC-11, CC-O4, SIM-04 |
| S-11 | `failure_message` joins full argv (gh issue bodies) into errors and the `pushes.results` ledger | SEC, CC | Low | read | S | Name tool + verb only, keep stderr's first line | SEC-12, CC-O3 |
| S-12 | Guard compares `Origin` host but not scheme (`https://127.0.0.1:3999` passes); `Host` match is case-sensitive while `Origin` is not | TEST, CW, SEC | Low | run | S | Compare scheme `http`; `eq_ignore_ascii_case` on Host; pin in the T-01 test | T1, CW-15, SEC-14 |

### Correctness

| ID | Title | Lenses | Sev | Ev | Eff | One-line fix | Sources |
|---|---|---|---|---|---|---|---|
| C-01 | `iter clone` with `base_path` inside the source copies itself until `ENAMETOOLONG` (176 dirs left, no project row); `.iter-worktrees/` copied | CC | **High** | run | S | Canonicalise both, refuse `dest.starts_with(src)` before `create_dir_all`; skip `.iter-worktrees` | C1 |
| C-02 | Confirmed share push is re-executable: `Check.already_pushed` computed and discarded, result is a 200 render so refresh re-POSTs; two ledger rows with one hash | CW, SIM | **High** | run | S | Return `Outcome::Preview{already:true}` when `already_pushed`; 303 to the report (`?pushed=<hash>`) after push/copy | CW-1, CW-6, SIM-06 |
| C-03 | Compose form offers and previews a `tasks/<id>/instructions/*.md` instruction, `POST /task/{id}/runs` refuses it (`layers_of` without the `Source::Task` layer) | CW | **High** | run | S | Build the layers once (incl. task layer) in `world()`; use them in `create_run`, `edit_run`, `prompt_of` | CW-2 |
| C-04 | `task done` without `--save` runs `git branch -D` on a branch with unmerged commits, silently (commit survives only in reflog) | CC | Medium | run | M | Check `merge-base --is-ancestor` first; refuse or keep the branch with a warning (`git.rs:69`, `workspace.rs:47`) | C3 |
| C-05 | Wiki lock check-then-remove race: two stale-lock finders both take it; `Drop` removes the other's lock | CC | Medium | read | M | `flock` on a long-lived file (as `taskdir.rs:481`) or verify pid ownership in `Drop` | C4 |
| C-06 | CLI stores task names untrimmed (`'task '` never resolves, duplicate created); `task pull` stores empty issue titles | CC | Medium | run | S | Trim in `checked_tags` like the `Named` derive; `require_name` on pulled titles | C5 |
| C-07 | Sessions bucketed by start day: 23:30→01:30 is 2 h on day one, 0 on day two, in report and pushed worklog | CC, TEST | Medium | run | M | Split at midnight when bucketing and in the range query, or document "belongs to its start day"; add the missing midnight tests (T-06) | C6, T7 |
| C-08 | Sessions in naive local time: on the autumn change a 30-min session reads −30 min and `too_short` deletes it | CC, TEST | Medium | read | M | Store UTC (or offset) and convert for display; never treat negative `end−start` as "short" | C7, T7 |
| C-09 | `any_client_attached`/`take_detach_message` pass a bare `-t name` (prefix match: `web/a` hits `web/ab`) | CC | Medium | run | S | Use `exact()` (`=name`) as every other target does | C8 |
| C-10 | `read_secret` runs `stty -echo` and never restores it on Ctrl-C | CC | Medium | read | S | Restore in a guard that also covers SIGINT (or `rpassword`) | C9 |
| C-11 | Platform reply parsing: any 2xx JSON object without `results` → every day "ok"; dates compared as strings so RFC-3339 dates disable the conflict check | CC | Medium | read | S | Require a `results` key for success; normalise `date`/`day` through `NaiveDate::parse_from_str` | C10, C11 |
| C-12 | Stale task edit form overwrites board-owned fields (start, duration, Urgent/Important) set by a drag since the form was opened | CW | Medium | run | M | Row version in the form and refuse a save over a newer row, or make board fields read-only on the edit page | CW-3 |
| C-13 | Run prompt textarea and session note keep CRLF (`\r` reaches the agent; `modal.js` loaded-compare misfires) | CW | Medium | run | S | `forms::text()` on `prompt` and `message` like every other textarea | CW-4 |
| C-14 | `modal.js` keeps the previous valid instruction in the hidden field when the prompt fetch fails: a typo queues the wrong instruction | CW | Medium | read | S | On failure clear `chosen.value` and show the status; or post the visible field | CW-7 |
| C-15 | CLI task insert and `set_task_tags` are two transactions (web uses `save_task` in `atomically`, `cfg(web)`) | CC | Low | read | S | Wrap in `db.atomically`; expose `save_task` without the feature (re-confirms Oct DRY F1) | C12 |
| C-16 | Ledger `results` lines not newline-stripped for FAILED/GitHub lines; a hostile message forges "pushed" days | CC | Low | read | S | Strip newlines as `conflict_line` does | C13 |
| C-17 | Only `??`/`A` count as new wiki pages: `AM` makes `settle(revert)` fail half-way | CC | Low | read | S | Treat any first-letter `A` as new, or `git reset` first | C14 |
| C-18 | `allowed_values` walks `rdf:List` without a visited set: a cyclic `ontology.ttl` hangs every wiki command | CC | Low | read | S | Visited set or max length | C15 |
| C-19 | A page without `updated` keeps its note dirty forever → an ingest agent per trigger | CC | Low | read | S | Treat missing `updated` as mtime, or lint it | C16 |
| C-20 | `restore_cmd` aborts on the first `create_session` failure: hooks and clocks for the others never handled | CC | Low | read | S | Warn and continue | C17 |
| C-21 | Ingest spawn creates the log before taking the lock: empty logs when busy | CC | Low | read | S | Lock first | C18 |
| C-22 | `create_indexes` doc comment wrong: indexes only created in `baseline`; a later index on a later column never lands on upgraded DBs | CC, TEST | Low | run | S | Fix the comment; add a migration step per new index; add the "resume from version 3" test (T-05) | C19, T6 |
| C-23 | `to=9999-12-31` → `+10000-01-01` sorts below every row: empty report | CW | Low | run | S | Clamp or `checked_add` guard in `range_bounds` | CW-8 |
| C-24 | `parse_run` trims `model` but not `effort`/`instruction` | CW | Low | run | S | `trim()` all three | CW-9 |
| C-25 | Amending a cancelled run says "has begun" | CW | Low | run | S | Say "was cancelled" | CW-10 |
| C-26 | `?run=N` looked up after `take(RUNS=50)`: old queued runs cannot be opened from the Agent tab | CW | Low | read | S | Look up before truncating | CW-11 |
| C-27 | `/agents` and `/instructions` swallow DB errors (`unwrap_or_default`) and render "No agent has worked…" | CW | Low | read | S | Propagate the error | CW-12 |
| C-28 | Write paths propagate `IterError` as 500 (`?`) or show raw `e.to_string()`; io errors and platform errors land on pages verbatim | CW, SEC | Low | read | S | Route through `errors::refused`/`FormError::sentence` (ties to W-04) | CW-13, SEC-13 |
| C-29 | `tilde()` has no separator check: `/Users/carlos/app` → `~s/app` | CW | Low | read | S | Require `/` after `$HOME` | CW-14 |
| C-30 | `board.js` has no in-flight guard: two quick drops send two POSTs | CW | Low | read | S | `busy` flag as `path_pick.js` | CW-16 |

### Performance

| ID | Title | Lenses | Sev | Ev | Eff | One-line fix | Sources |
|---|---|---|---|---|---|---|---|
| P-01 | `shown_status` spawns `kill -0` + `ps` per record marked running: `/agents` 103→418→1698 ms at scale 1/4/16 (97 % of the page); stale `running` records cost 5 ms each forever; same code path as S-03 | PERF, CC, SIM | **High** | run | S | One `ps -o pid=,stat= -p a,b,c` per request (or `libc::kill(pid,0)`, no spawn), memoised per render; fix together with S-03 | P1, CC obs, SIM obs |
| P-02 | `/search` rebuilds the index (4 scans + read and YAML-parse of every daily note) per keystroke: 19→80→150 ms for 971→3884→6776 notes | PERF, CC, CW | **High** | run | M | Cache `Vec<Entry>` per `serve` process; invalidate on `PRAGMA data_version` + notes folder mtimes (or 2 s TTL) | P2, CC obs, CW obs |
| P-03 | `/agents`, `/instructions`, `/instruction/{name}` walk every task folder (`usage()`): N+1 org lookups, a `read_dir` and R YAML parses; 3.6→12→42 ms | PERF, CW | Medium | run | S | Load orgs once; skip projects without a notes folder; count entries, do not parse, on the instructions pages | P3, CW obs |
| P-04 | `ingest::auto` builds the whole oxigraph store before reading `settings.yaml` (default `manual` discards it) on `session new`, `task done`, `solve begin`: +90 ms at 2000 pages | PERF, CC | Medium | run | S | `scope::levels` + `ingest::mode` first; build only when the trigger is wanted | P4, CC obs |
| P-05 | `/task/{id}?tab=wiki` builds the graph + 3 `git` spawns per render (122 ms at 2000 pages); `?new=agent` builds it for the prompt (94 ms) | PERF | Medium | run | M | `in_git`/`changes` once per render; compute `waiting` from front matter without filling the store; or cache the `Wiki` per notes dir behind mtimes | P5 |
| P-06 | 2–3 `Db::open` per page (page, footer, third in agent tab and edit pages); 0.6 ms each vs 4 µs per query; `migrate()` is one pragma read (seed's implied cost refuted) | PERF, CO, CW | Low | run | S | One `Db` per request (thread-local or `App` built by a layer, CO-C12); the footer reads from it | P6, CO-C12, CW obs |
| P-07 | Daily-note page reads and parses every note to find prev/next, then reads the note again (7.7 ms at 365 notes) | PERF | Low | run | S | Neighbours from file names (`read_dir` + `note_day`) | P7 |
| P-08 | Badge counts parse every run YAML / read every session or project row for a number | PERF | Low | read | S | Count entries; `SELECT COUNT(*)` | P8 |
| P-09 | `push::plan` and `restore_cmd` N+1 PK lookups (4 µs each; `tmux has-session` spawn dominates `restore`) | PERF | Low | read | S | Keyed maps from the rows already loaded | P9 |
| P-10 | Worklog O(D·S) with `Session` clones; measured flat (30 ms at 3200 sessions = process floor) | PERF | Low | run | S | Index tasks by day once; borrow instead of clone | P10 |
| P-11 | `/agents` is unbounded (346 KB at scale 16); `?tab=notes` lists every note (132 KB) | PERF | Low | run | S | Cap like `RUNS = 50`; paginate notes | P obs |
| P-12 | `/search.js` loaded synchronously before content (`frame.rs:47`) delays first paint | A11Y | Low | run | S | `defer` | A11Y obs |

### Web design and accessibility

| ID | Title | Lenses | Sev | Ev | Eff | One-line fix | Sources |
|---|---|---|---|---|---|---|---|
| W-01 | `/` opens the palette page-wide with no off switch or remap (WCAG 2.1.4, Level A). Rubric-literal Blocker; practical harm small (editable fields excluded) | A11Y | **Blocker** (rubric; practical High) | run | S | Settings toggle (`localStorage` flag read by `search.js`) or fire only when the Search button has focus | A11Y-01 |
| W-02 | Top bar never wraps: every framed page is 654 px wide at 320/360/400 px, "Instructions", "Settings", Search off-screen (1.4.10; regression since `0a3f175` 2026-10-31) | DC, A11Y, BP | **High** | run | S | `@media (max-width:600px){.bar{flex-wrap:wrap;height:auto}}` (and `--bar-h` must follow: sticky offsets read it) or hide labels under 600 px with `aria-label`s | DC-01, A11Y-02, BP |
| W-03 | Projects table 1099 px wide under 600 px (org Projects tab, board Info): `.path-cell` loses `max-width:0` while the row is still a table row | DC, A11Y, BP | **High** | run | S | Keep `max-width:0`/`min-width:0` on `.path-cell` under 600 px, or `table-layout:fixed;width:100%` | DC-02, A11Y-03, BP |
| W-04 | Refused New note / New session / New agent / New instruction / instruction save answer `400 text/plain "bad request: …"`: typed form lost, no title/landmark/link, lowercase fragment, contradicts `doc/web_server.md`; `required` absent so Enter gets there | CW, DC, A11Y, SEC | **High** | run | M | Re-render the pop-up with `FormError::sentence` as `task_new.rs:188-231` does; add `required`; decide whether `errors::error_pages` also dresses POST 400s (T-02 test depends on it) | CW-5, DC-04, A11Y-07, SEC-13 |
| W-05 | Footer project links 17 px tall, 3 px apart: axe `target-size` on 74 of 98 runs (2.5.8) | A11Y | **High** | run | S | `.foot-sub a{display:inline-block;padding-block:4px}` or `li{min-height:24px}` | A11Y-04 |
| W-06 | `--muted` on `--accent-tint` = 4.32 light / 4.14 dark (selected palette subtitle, active combobox hint, `.drop.over .empty-line`); Oct A6 fix insufficient (1.4.3) | A11Y | **High** | run | S | Those three selectors → `var(--fg)`, or darken `--muted` to ~`#67635c` / `#9fa2b2` (also lifts `--muted` on `--bg-3` from 4.33) | A11Y-05 |
| W-07 | "Go to this date" `button.sr` is a Tab stop with no visible focus (clip-path) (2.4.7) | A11Y | **High** | run | S | Un-clip on `:focus` like the report's Apply (`org.css:18-19`), or drop it when script runs | A11Y-06 |
| W-08 | Six dismiss idioms: Cancel `a.u`, ✕, `a.button` Close (next to ✕ in New agent), `a.button` Back, `a.button.small` Cancel, `.step-close` (Oct H1 + two new) | DC, A11Y | Medium | run | S | One rule: Cancel is the `.u` link in forms and pop-ups, ✕ only on modals; drop Back/Close/`.step-close` | DC-03, A11Y obs |
| W-09 | Closing a pop-up (✕, Cancel, Esc, backdrop) navigates to the list URL and focus lands on `body`, not the "New …" opener (2.4.3) | A11Y | Medium | run | S | Give the add row an id and append `#new-x` to the close URLs (`task_new.rs:109-112`, `task_notes.rs:32`, `task_sessions.rs:31`, `instructions/save.rs:23`, `org.rs:122-126`); pick mode already does this | A11Y-10 |
| W-10 | Nine primary labels (Save, Create task, Create project, Create, Add session, Add note, ▶ icon-only Run now, Copy, Push); the only icon-only primary sits next to two labelled secondaries; compose form lacks "* Required" | DC | Medium | run | S | "Create X" or "Add X" for every New pop-up; visible label on Run now; build compose through `form_shell` | DC-05 |
| W-11 | Share dialog: Copy/Push modality toggle looks like bordered buttons above a primary "Copy": two Copy buttons | DC | Medium | run | S | Segmented control (reuse `.presets`), one primary submit | DC-06 |
| W-12 | Tab system disagrees with itself: note editor tab "Edit" vs "Settings" elsewhere; board edit `aria-label="Sections"` vs "Board sections"; tab "Agents" vs title "Agent" | DC | Medium | run | S | One naming and one `aria-label` per tab row | DC-07 |
| W-13 | 33 empty-state wordings; 8 hand-written `<p class="empty-line">`, six carrying notes or errors ("The front matter cannot be read: …") in the "nothing here" style | DC, SIM | Medium | run | S | `.empty-line` only for "nothing here"; notes/errors through `notice`; list the allowed set in `doc/web.md` | DC-08, SIM refuted-list |
| W-14 | `/agents` and `/instructions` are `data_table`s without the `filter_bar` the task/project lists have; `/agents` will be the longest list | DC, BP | Medium | run | M | Add `filter_bar` to both | DC-09, BP |
| W-15 | 400/404 pages: brand only, no nav, Search, footer or eyebrow; reads as a different site (a11y-wise fine: title, skip link, one `main`) | DC, A11Y, BP | Medium | run | S | Build through `frame` | DC-10, A11Y-21, BP |
| W-16 | Three kicker conventions: eyebrow "Organization"/"Global"/"Board" (over h1 "Boards"), crumbs on project/task, section-as-last-crumb on boards (Oct H3 half open) | DC, BP | Medium | run | S | Eyebrow matches the h1; crumbs for every entity page incl. Organization; nav "Boards" | DC-11, BP |
| W-17 | New task pop-up has 9 fields, scrolls inside at 900 px, Create/Cancel below the fold (Oct M3) | DC, BP | Medium | run | M | Project, Name, Description only; the rest on the Settings tab | DC-12, BP |
| W-18 | Palette options are `<li role=option><a>`: axe `nested-interactive` ×50, Chrome exposes links so `aria-activedescendant` targets lose the option role (4.1.2) | A11Y | Medium | run | S | `li` is the option (`data-href`), navigate on Enter/click; or a list of links with roving focus | A11Y-08 |
| W-19 | Disabled period arrows are `span`s with a prohibited `aria-label` (4.1.2) | A11Y | Medium | run | S | `<a role=link aria-disabled>` or `<button disabled>` | A11Y-09 |
| W-20 | Daily-note pages carry two `aria-current="page"` (crumb and tab); Oct A1 regressed there | A11Y | Medium | run | S | `Crumb::label` when the page has tabs | A11Y-11 |
| W-21 | Card focused after Cancel/move has `outline:none` though `:focus-visible` matches (Oct F11) | A11Y | Medium | run | S | `.tcard:focus:not(:focus-visible){outline:none}` | A11Y-12 |
| W-22 | Six date shapes (crumb long, body long weekday, h1 "02/11", tab "Sun 1 Nov", ISO tables, native `27/10/2026`) | DC | Low | run | S | Short `%a %-d %b` in lists, long in headings, ISO only in mono tables | DC-13 |
| W-23 | 16 computed font sizes from `em`-on-`rem` nesting; `design/04` §1 says "not built" | DC | Low | run | M | Four `--t-*` tokens | DC-14 |
| W-24 | `<title>` and three `<script>`s emitted inside `<body>` (Oct F7) | DC, A11Y | Low | run | M | Render in `<head>` (errors.rs already does) | DC-16, A11Y-18 |
| W-25 | `class="button…"` spelled out in 8 files incl. three copies of the Run/Save/Queue/Close row; SIM: not a component miss (no plain button component exists) | DC, SIM | Low | run | S | Optional `button(label, kind)` in `ui/button.rs`, worth it only as the single place for the W-10 label rule | DC-17, SIM refuted |
| W-26 | Task with empty description: Info tab is a right panel and nothing on the left | DC | Low | run | S | "No description." `empty_line` | DC-18 |
| W-27 | Wiki tab prints an absolute server path and "Run `git init`" in an empty-line; "No sketches yet." has no section label | DC | Low | run | S | Shorten, drop the path, label the section | DC-19 |
| W-28 | Board edit refusal lacks `role="alert"` (org edit has it) | DC | Low | run | S | Same `notice(error: true)` | DC-20 |
| W-29 | Pick banner's 4 px edge overridden by `.notice{border:1px}` (sheet order) | A11Y | Low | run | S | Move `pick_bar.css` after `notice.css` | A11Y-13 |
| W-30 | `/instruction/develop`: h2 then h4s from the prompt markdown | A11Y | Low | run | S | Demote prose headings relative to the section | A11Y-14 |
| W-31 | New agent dialog opens without `autofocus` (others do) | A11Y | Low | run | S | `autofocus` on the first field | A11Y-15 |
| W-32 | Date being typed sits in a 1×1 transparent input: three invisible Tab stops (Oct F2 remainder) | A11Y | Low | run | M | Visible date field under 900 px | A11Y-16 |
| W-33 | Card action link invisible until hover/focus on desktop (Oct F10) | A11Y | Low | read | S | Always-visible affordance | A11Y-17 |
| W-34 | `h2` inside `<summary>` exposed as disclosure, not heading (Oct A7, accepted) | A11Y | Low | run | S | Accept or move the heading | A11Y-19 |
| W-35 | Pill/search-button/chip hairlines 1.4–2.8:1 (decorative, text identifies them) | A11Y | Low | run | S | `--control-line` → 3.5:1 | A11Y-20 |

### Simplification and structure

| ID | Title | Lenses | Sev | Ev | Eff | One-line fix | Sources |
|---|---|---|---|---|---|---|---|
| X-01 | "Organization of a project" written 8× in two variants; two swallow DB errors and pick a different notes folder | SIM | Medium | read | S | `Db::org_of(&Project) -> Result<Option<Organization>>` in `db/groups.rs` | SIM-01 |
| X-02 | Front matter split in 7 places with 4 rules (BOM, `\n---\n` vs line-exact divider, open-ended front) | SIM, CO | Medium | read | S | One `md_edit::split_front`; also breaks the `instruction <-> prompt` cycle (CO-C9) | SIM-02, CO-C9 |
| X-03 | Push flow written once for CLI and once for web; `push::resolve` called twice per push (two platform round-trips) | SIM, CW | Medium | read | M | `push::prepare(...) -> (Plan, Check, Resolved, warnings)`; `execute` takes `Resolved`; natural home for S-02's re-check | SIM-06 |
| X-04 | "Task with a session cannot move" and "rename the API key with the org" each written twice across the seam, already drifted in wording | SIM | Medium | read | S | `Db::move_task`, `secret::rename_for(db, id, old)` | SIM-07 |
| X-05 | Agent compose form written twice (`agents.rs:255`, `task_runs.rs:281`), drifting (Close button, hints, `loaded:`) | SIM, CW, DC | Medium | read | M | One `compose_form(...)` component; also where W-08/W-10 land | SIM-15, CW obs, DC-03/05 |
| X-06 | ~700 lines of web-only code compiled into the CLI binary without a `cfg(feature="web")` gate: 36 dead-code warnings on `--no-default-features` | SIM, CO, TEST | Low | run | S | `#[cfg(feature = "web")]` on `agent/instruction` editing half and the listed items; CI job on `--no-default-features` | SIM-20, CO-C3, T11 |
| X-07 | Three faces of "run a tool" (`process` free fns, `Tool`, `Runner` re-implementing `run`/`output_trimmed`); `tmux.rs:235-247` free fns bypass `Multiplexer`, so `project delete`, clock and hook paths cannot use `FakeRunner` | SIM, CO | Low | read | M | `Runner` defaults call the free fns; delete the tmux free fns and carry `&dyn Multiplexer` on `App`; enables tests for S-08/C-09 | SIM-03, CO-C4 |
| X-08 | Page-overlay cycle grown to 7 modules (`org`, `org_report`, `org_share`, `project`, `project_new`, `task_new`, `board_info`); `recursion_limit = 256` is its symptom | CO | Low | run+read | M | Pop-ups as pure views taking the host's `screen`; `Home`/`TaskHome`/`*Create` into `web/create.rs` | CO-C1 |
| X-09 | Core cycle `config -> agent/taskdir -> notes -> md_edit -> config` via `write_atomic`; `models -> scaffold -> config` via `absolute_path` in `Project::validate` | CO | Low | run+read | S | `utils/fs.rs::write_atomic` (callers: `config.rs:144`, `notes.rs:204,249`, `wiki/mv.rs`, `instructions/save.rs:92,170`); resolve the path in callers | CO-C2 |
| X-10 | Small cycles: `forms <-> edit` (NAME/DESCRIPTION), `ui/field <-> group <-> notice`, `url <-> sections`, `task_agent <-> task_runs`; hard-coded URL at `task_agent.rs:285`; `run_url` ≡ `agent_url` | CO, SIM | Low | run+read | S | Move the two consts, `ui/field_ids.rs`, `NoteTab` into `url.rs`, `Roster` into `task_agent`; `agent_tab_url(&task_url(id))` | CO-C5..C8, SIM-18 |
| X-11 | Web plumbing: 30 `register` fns + 10 four-line asset routes (~210 lines), JSON header ×4, six note handlers in three pairs | SIM | Low | run | M | `json()` helper; asset table via a `Route` impl; `Owner` generic for notes; `register` only if topcoat `discover` is accepted (+1 crate) | SIM-12, 13, 14, 17 |
| X-12 | Dead CSS: `.plan-progress`, `.rail-sum`, `.run-page`, `.prompt-form/.prompt-from`, `.share-check`, all of `tasks_tab.css` (`.lost` is live) | SIM | Low | run | S | Delete (~15 lines) | SIM-19 |
| X-13 | 14 functions > 80 lines (`commands/wiki.rs:36` 167, `wiki/check.rs:94` 161, `push.rs:305` 114, …); god modules `org_share.rs` 665, `task_runs.rs` 543 | SIM, CO | Low | run | M | Split by responsibility; `org_share` → dialog + push bridge; `task_runs` → roster/compose/run page | SIM-23, CO-C10 |
| X-14 | Tiny wrappers and pedantic lints: `with_timeout` builder copy, `Job` built twice, `Wiki::node` alias, `sessions_for_project_in_range` one caller, 34× `push_str(&format!())` | SIM | Low | run | S | Fold/delete; `write!` | SIM-04, 05, 08, 10, 11, 22 |
| X-15 | JS: date-apply pattern ×2 (differing), fetch-chain ×3, debounce ×5; no shared module | SIM, TEST | Low | read | M | Only with a `common.js` asset; export helpers for tests (T-07) | SIM-21, T8 |
| X-16 | `config()` global read from 13 modules; `App` hand-built in 3 web places (no non-test constructor); pages open their own DB (cause of P-06) | CO | Low | run+read | S | `App::over(db, now)`; config and tmux as `App` fields | CO-C12 |
| X-17 | 96 orphan `pub` items; 4 one-use wrapper components; `Table` trait lives in `db` though `models` derives it | CO | Low | run | S | `pub(super)`; inline `matrix_grid`/`info_body`/`board_root`/`side_lists`; `models/table.rs` | CO-C13, C14, C15 |

### Tests

| ID | Title | Lenses | Sev | Ev | Eff | One-line fix | Sources |
|---|---|---|---|---|---|---|---|
| T-01 | `guard::local_only` layer never executed by a test; pure tests miss scheme (run: `Origin: https://…` passes), IPv6, portless host, `Origin: null`, HEAD/OPTIONS; topcoat's `OriginPolicy` hides the guard's origin branch over HTTP | TEST, SEC | **High** | run | S | In-process `Router::handle` test with `OriginPolicy::dangerous_disable()` (proof in `doc/review_tools/testing/testproof/`); pin S-12 | T1 |
| T-02 | Zero tests through `router()`: layers, `error_pages` (HTML vs text for POST), `path_param!` 400, JSON routes | TEST | Medium | run | M | `#[cfg(test)]` harness over a temp DB (`XDG_CONFIG_HOME` before first `config()`); write it after W-04 settles the POST-400 contract | T2 |
| T-03 | ~12 tests read the developer's real `iter.yaml` through `config()` (`pause_gap_minutes` flips fingerprints; an invalid file `exit(1)`s the binary) | TEST | Medium | read | S | Pass the gap explicitly or set `XDG_CONFIG_HOME` in fixtures | T4 |
| T-04 | `edit.rs::settle/submit/create_to` (every form's save-or-refuse) and `drop_on` 404/400 paths untested (Oct M4/M5) | TEST | Medium | read | M | Fake `EditForm` tests; DB-backed `drop_on` tests | T5 |
| T-05 | Migration resume from a middle `user_version` never tested; steps 4–6 only run after baseline | TEST, CC | Medium | read | S | Test from `user_version = 3` asserting steps 4–6 run once | T6, C19 |
| T-06 | No session crossing midnight, no DST day, no leap day in merge/worklog/clock tests; `now()` not injectable | TEST, CC | Medium | read | M | Add 23:50→00:10 and DST-day cases (proves C-07/C-08 fixes) | T7, C6, C7 |
| T-07 | 8 JS files, 1043 lines, 0 tests; helpers are closures inside IIFEs | TEST | Medium | run | M | Export helpers (`window.iter = {…}`); adopt `doc/review_tools/testing/js/run.sh` (node:test + jsdom, proven) | T8 |
| T-08 | No render-through-component escaping lock-in (Oct M2, downgraded: escaping is by construction) | TEST | Low | run | S | One parametrised `view!` test with `<img onerror>"&` | T3 |
| T-09 | Redundant tests: two `Harness::Claude.command` tests, five `round_to_half_hour_*`, duplicate hex-colour test, three `*_errors_name_their_field` | TEST | Low | read | S | Merge | T9 |
| T-10 | `alive`/`interrupt`/`shown_status "lost"` untested (incl. `pid 0`) | TEST, SEC | Low | read | S | `FakeRunner` tests; part of S-03 | T10 |
| T-11 | No non-ASCII title test for search marks (`İ` changes length on lowercasing) or unicode in `task_query` | TEST | Low | read | S | Add cases | T12 |
| T-12 | Two tests read the wall clock after the code under test did | TEST | Low | read | S | Fix the clock | T13 |
| T-13 | 116 zero-test files (~40 `web/ui/*`, 7 `commands/*`) | TEST | Low | run | L | Render-to-string tests for frame, card, field via `ViewHandle::render` | T14 |

### Build and size

| ID | Title | Lenses | Sev | Ev | Eff | One-line fix | Sources |
|---|---|---|---|---|---|---|---|
| B-01 | No distribution profile: 25.6 MB unstripped / 18.5 MB stripped; fat LTO on `release` would make incremental 7 s → 230 s | BUILD | Medium | run | S | `[profile.dist]` inherits release, `opt-level="s"`, `lto="fat"`, `codegen-units=1`, `strip=true`, `incremental=false` → 12.1 MB, 213 s clean, tests green on fat LTO; `panic="abort"` ruled out (topcoat `catch_unwind`) | F1 |
| B-02 | `Iter` crate is 57.2 s of 110.9 s clean wall (52 %), 75 % backend; `topcoat_view` generics + `Iter` = 34 % of `__text` | BUILD | Low | run | L | Type-erase at page boundaries; fewer generic `view!` instantiations; keep `incremental = true` | F2 |
| B-03 | `syn 2` pulled only by `macros/`; compiles unchanged on `syn = "3"` | BUILD | Low | run | S | Bump | F3 |
| B-04 | No `[profile.dev]`: debug clean 53.7 s, `target/` 2.2 GB | BUILD | Low | run | S | `debug = "line-tables-only"` + `[profile.dev.package."*"] debug = false` → 45.2 s, 1.7 GB; do not add `opt-level=1` for deps | F4 |
| B-05 | sqlite amalgamation (26 s) is the serial step before `Iter` | BUILD | Low | run | S | Keep `bundled` for shipping; optional feature to use system sqlite in dev (105 s clean) | F5 |
| B-06 | topcoat `serve` hard-depends on `tokio-tungstenite` (dev websocket client) and hyper `http2`; forces the `sha1 0.11` workaround comment | BUILD | Low | read | S | Upstream issue; nothing fixable here | F6, F8 (clap_complete trim confirmed impossible) |

### Docs stale — see §7 (8 items, all Low, S).

## 3. Conflicts between lenses, resolved

| # | Conflict | Decision | Why |
|---|---|---|---|
| 1 | `*_edit.rs` skeleton: SIM-16 "acceptable, re-confirmed Oct F11" vs CO-C5 wants `NAME`/`DESCRIPTION` moved out of `forms` | Keep the skeleton; do the two-const move only | SIM judged duplication (none worth a generic); CO judged the cycle (`forms <-> edit`). Both true. The move is 2 lines and breaks the cycle; the generic handler is not worth it. Low, batch C. |
| 2 | Six dismiss idioms (DC-03) vs focus return on close (A11Y-10) | One change: one idiom *and* a `#new-x` fragment on every close URL | Not a contradiction: the same five close-URL sites. Doing one without the other re-opens the file. Batch D, one commit. |
| 3 | Hand-written `class="button"`: SIM refutes "component miss" (no plain button component exists) vs DC-17 wants one and DC-05 flags nine labels | Labels (W-10) Medium and mandatory; component (W-25) Low and optional, justified only as the one place to enforce the label rule | SIM is right that a class is not a miss; DC is right that the labels are inconsistent. The compose-form merge (X-05) removes two of the three copied button rows anyway. |
| 4 | TEST-O1: topcoat's `OriginPolicy` already denies cross-origin non-GET before layers; guard's origin branch unobservable over HTTP. What for the security fix? | (a) CSRF is doubly guarded — keep the guard, it adds what topcoat lacks (Host check = DNS rebinding, `Origin == Host`). (b) Irrelevant to S-01: a framed form POST is *same-origin*, so both checks pass; only headers + `Sec-Fetch-Dest` close clickjacking. (c) Neither compares scheme (S-12). (d) The T-01 test must `dangerous_disable()` the policy to reach the guard. (e) Document the two layers in `doc/web_server.md`. | Re-run: `POST /nope` with `Origin: http://evil.example` → 403 (topcoat, unmatched route); `Origin: https://127.0.0.1:3999` → 404 (passes both). |
| 5 | Seed "every page opens the DB twice, each open runs PRAGMAs, WAL and `migrate()`" vs PERF "0.6 ms per open, `migrate()` is one pragma" | Seed partly confirmed: 2–3 opens per page, 30–50 % of a 1.5 ms page; the implied heavy `migrate()` is refuted (`migrate.rs:12` returns on `user_version`). Low (P-06), fixed structurally via `App` per request (X-16), not as perf work. | Measured, cause isolated. |
| 6 | A11Y-01 `/` shortcut: rubric says Blocker (Level A), lens says practical High | Listed as Blocker per rubric, flagged; ordered in the backlog *after* the Highs with user impact because the fix is a toggle (S) and the harm is narrow | The brief's rubric is explicit; honesty about the gap beats silently re-rating. |
| 7 | SEC-01 Blocker vs the lens' own "High/Blocker boundary" note | Blocker | Run-confirmed by two independent passes (SEC, BP); the framed action is a one-click `acceptEdits` agent launch in the project checkout. |
| 8 | DC-04 "`POST /task/new` re-renders (seed refuted)" vs CW-5 lists `task_new.rs:177,217` as bare 400 | Both right for different inputs: validation errors re-render; a non-numeric or unknown `project` id is a bare 400. Seed "partly". | Read `task_new.rs`; both lenses ran it. |
| 9 | Error pages without nav: DC-10 Medium vs A11Y-21 Low | Medium (W-15) | A user notices; a11y-wise the page is complete, so the a11y rating is not wrong, just a different lens. |
| 10 | Web-only code in the CLI build: CO-C3 Medium vs SIM-20/T11 Low | Low (X-06) | Cosmetic maintenance per the brief; three lenses converging raises confidence, not severity. Moves into batch E because gating shrinks the CLI binary. |
| 11 | S-03 (security: validate pid) vs P-01 (perf: batch probe) touch the same `run.rs:169-212` | One batch, one commit: validate, then probe once | Sequencing: validation first (defines which pids are probed), then the batched `ps`. |
| 12 | Composability wants `Roster` moved (CO-C8), simplification wants the compose form merged (X-05): both in `task_runs.rs` | Merge first (X-05), then move (CO-C8) | Moving a form that is about to be rewritten wastes the move. |
| 13 | `git.rs:529` cited by CC-C3 does not exist (file is 124 lines) | Claim holds at `src/git.rs:69` (`branch -D --`) | Citation error only. |

## 4. Seed findings scorecard

| Seed | Verdict | By | Evidence |
|---|---|---|---|
| `Organization::check` accepts `http://localhost:1@evil.com/` | **confirmed** | SEC, CC | SEC-02 saved it via `/org/1/edit`, sink saw `authorization: Bearer dummy`; CC-O1 via `iter org new` |
| … check runs only on write; `platform.rs:147` does not re-check | **confirmed** | SEC, SIM | `platform_pick::open`/`JsonWebhook::new` trust the stored string (read) |
| … confirm ureq `RedirectAuthHeaders` default | **refuted as a risk** | SEC | ureq 3.5.0 `Config::default()` = `Never` (`config.rs:1075-1077`); `Authorization` dropped on redirect |
| No XFO/CSP/nosniff/Referrer-Policy; framed page can clickjack Run now etc. | **confirmed** | SEC, BP | `curl -sI` shows four headers only (re-run today); PoC renders `/task/1` in a cross-origin iframe (two screenshots) |
| `search.js:32` frames own pages → `SAMEORIGIN` not `DENY` | **confirmed** | SEC, A11Y, TEST | read; palette preview is an `<iframe>` |
| `GET /org/{id}/platform-activities` outbound call on cross-site GET | **confirmed** | SEC | sink saw `GET /projects` with bearer on one cross-site GET |
| tmux hook names positional, `-` parsed as flag | **confirmed** | SEC, CC | `$BIN internal hook session-closed -web/a` → exit 2 (re-run today) |
| Agent rewrites `runs/<n>.yaml`; `pid: 0` signals iter's own group | **confirmed** | SEC (kill semantics run), CC, PERF, TEST (read) | `kill -0 0` exit 0 (re-run); Python `setpgrp` + `kill -TERM -- -0` died 143; `--add-dir dir.root()` and `runs_dir = root/runs` read |
| `failure_message` echoes argv into errors and ledger | **confirmed**, Low | SEC, CC | read `process.rs:19-25`, `push.rs:289-295,398-406` |
| clone copies `.iter-worktrees/` and recurses forever | **confirmed** | CC | 176 nested dirs, `ENAMETOOLONG`, no project row |
| Raw error strings on pages; markdown `//host` links and remote images | **confirmed** | SEC | `safe_url("//…")` passes; headless Chrome fetched the pixel with `Referer` |
| `task_runs.rs:410,473` validate without the task layer | **confirmed** | CW | preview 200, POST 400 `'review' is not an instruction` |
| DB opened twice per page, PRAGMAs/WAL/`migrate()` each time | **partly** | PERF | 2–3 opens confirmed at 0.6 ms each; `migrate()` is one `PRAGMA user_version` (not a cost) |
| `/search` rebuilds index and reads every notes folder per keystroke | **confirmed** | PERF, CC, CW | 150 ms at 6776 notes, bare seed 11.6 ms |
| Wiki graph rebuilt: `scope::wiki` per `iter wiki` cmd; `ingest::auto` before mode; task-runs page per render | **confirmed** (CLI part inherent per process) | PERF | +90 ms per build at 2000 pages; `auto` reads mode after `Target::new` (`ingest_job/mod.rs:104-107` re-read) |
| `shown_status` spawns `kill`+`ps` per running run, twice | **confirmed** | PERF, CC | `/agents` 1698 → 43 ms after marking records done |
| `push::plan` N+1, `restore_cmd` N+1, `notes::list` reads every file, worklog O(days×tasks×sessions) | **shapes confirmed, cost refuted** | PERF | 4 µs lookups; worklog 30 ms flat at 3200 sessions; `notes::list` is the P-02/P-07 cost |
| org-of-project copy-pasted ~7× | **confirmed, 8** | SIM | grep re-run today: 8 sites |
| Two runner abstractions + free fns | **confirmed** | SIM, CO | `Runner` re-implements `run`/`output_trimmed` (re-read `process.rs:145-165`) |
| Front-matter splitting ×4 | **confirmed, 7** | SIM | 4 distinct rules |
| `read_dir+filter+sort` ×4 | **not assessed** | — | no lens enumerated it (11 `read_dir` sites exist) |
| Editor temp-file code ×2 | **confirmed** | SIM, SEC | `md_edit.rs:65,109` |
| 30 `register` fns + 10 asset routes | **confirmed** | SIM | clone pair #1 and #2; judged a chosen trade-off (Low) |
| JSON header ×4 | **confirmed** | SIM | |
| Agent compose form ×2 | **confirmed, drifting** | SIM, CW, DC | Close button only in one |
| 4 edit modules same skeleton | **confirmed, acceptable** | SIM | re-confirms Oct F11 |
| URLs built outside `url.rs` in 9 places | **partly** | SIM, CW | one real hard-coded path (`task_agent.rs:285`); the rest are suffixes on a `url::` base |
| Dead CSS list | **confirmed in full**; `.lost` is live | SIM | `css_dead_classes.py` |
| "dim current on hover" rule ×3 | **refuted** | SIM | three different rules (`::before` transform / opacity / `::after`) |
| JS date-apply duplicated, fetch-chain ×3 | **confirmed**, differing | SIM, TEST | |
| Functions > 80 lines list | **confirmed exactly**, 14 | SIM | `fn_lengths.py` |
| Close/cancel idiom differs in 5 places | **confirmed, now 6** | DC, A11Y | screenshots |
| POST refusals bare-text 400 for notes/sessions/runs/instructions/new-task; skip `FormError::sentence`; `org_share` renders not redirects | **confirmed** for notes/sessions/runs/instructions; **partly** for new-task (unknown/non-numeric project only); `org_share` confirmed | CW, DC, A11Y | curl on three private stores |
| ~35 empty-state wordings; 8 hand-written empty-line; `class="button"` in 8 files; no filter bar on `/agents`,`/instructions`; errors page lacks frame; A6b and `/v2.css` stale | **confirmed** (33 wordings; 8 files / 28 sites; filter bar; frame); "component miss" **refuted** by SIM; docs stale confirmed | DC, SIM, A11Y, BP | |
| 297 lock packages; 223 / 147 crates | **confirmed** | BUILD | |
| topcoat pulls 114 incl. tungstenite and h2 despite `default-features=false` | **confirmed** (tungstenite/h2 not feature-gated upstream) | BUILD | registry sources read |
| oxigraph 74, zerocopy 15 MB rlib; ureq 29 (ring) | **not re-measured** (BUILD counted oxigraph at 29 platform-filtered packages, ~1.2 MB text; ring confirmed via rustls) | BUILD | |
| clap_complete `unstable-dynamic` costs 22 crates | **refuted as a delta** | BUILD | only `clap_complete` + `is_executable` would vanish; removing the feature breaks 105 sites |
| Duplicates syn 2/3, sha1/digest, base64, getrandom, hashbrown, rand 0.8/0.9 | **confirmed** except rand (no 0.8 present) | BUILD | `cargo tree -i syn@2` → `iter-macros` only (re-run today) |
| Binary 25.7 MB, `__LINKEDIT` 8 MB, 57k symbols, grew 4 MB | **confirmed** (25.6 MB / 26 917 392 B; strip → 18.5 MB); growth not assessed | BUILD | |
| Only `incremental = true`; no `--timings` ever run | **confirmed** | BUILD, BP | timings now at `target/cargo-timings/cargo-timing.html` |
| topcoat `catch_unwind` → `panic="abort"` unsafe | **confirmed** | BUILD | `topcoat-router body.rs:112`, `router.rs:100`, `memoize/cache.rs:311` |

## 5. Spot-check (top 3 claims per report, re-verified today)

| Report | Claim | Result | Evidence |
|---|---|---|---|
| security | SEC-01 no frame/CSP headers | held | `curl -sI 127.0.0.1:3999/task/1` → `content-type`, `vary`, `content-length`, `date` only |
| security | SEC-02 `starts_with` prefix check accepts userinfo | held | `organization.rs:69-74`: `strip_prefix("http://localhost")` then `rest.starts_with([':','/'])` |
| security | SEC-04 `kill -TERM -- -<pid>` trusts yaml pid; `kill -0 0` true | held | `run.rs:207-212` re-read; `kill -0 0; echo $?` → 0 |
| correctness_core | C1 no containment check in clone | held | `scaffold.rs:26-38` recurses into `read_dir(src)`; `project.rs:101-105` `create_dir_all(dest)` then copy |
| correctness_core | C2 hook name `-web/a` → clap exit 2 | held | re-run: `error: unexpected argument '-w'`, exit 2 |
| correctness_core | C3 `git branch -D` on `task done` | held, wrong line | cited `git.rs:529`; file has 124 lines; call is `git.rs:69` `["branch","-D","--",branch]` |
| correctness_web | CW-1 `execute` unconditional, `already_pushed` unused | held | `org_share.rs:298-300`; grep: `already_pushed` read only for display at `:312` |
| correctness_web | CW-2 layers without task layer on POST | held | `task_runs.rs:409-410,473-474` vs `:432-435` and `task_agent.rs:110-113` |
| correctness_web | CW-5 POST errors stay text | held (read) | `errors.rs:27-29` returns early unless GET/HEAD; `task_notes.rs:94` `map_err(bad_request)`; not re-POSTed (shared server kept read-only) |
| performance | P1 `alive` spawns `kill` then `ps` per call | held | `run.rs:197-204` |
| performance | P2 `entries(&open_db()?)` per `/search` request | held | `search_index.rs:159`; shared scale-4 store `/search?q=task` 11.9 ms (matches bare-seed order) |
| performance | P6 two opens, `migrate()` one pragma | held | `mod.rs:78`, `footer.rs:59` both `open_db()`; `migrate.rs:12` early return |
| simplification | SIM-01 eight org-of-project sites | held | grep `organization_id` match sites: 8 |
| simplification | SIM-02 seven front-matter splits | held (sampled) | `md_edit.rs:22-36`, `notes.rs:80-84` differ (BOM/`opened`); others per report |
| simplification | SIM-03 `Runner::run`/`output_trimmed` re-implement free fns | held | `process.rs:145-165` |
| composability | C1 `org_share -> org` edge | held | `org_share.rs:5 use super::org::{Loaded, load as load_org, screen}` |
| composability | C2 `write_atomic` in `agent/taskdir` called from `config`, `notes`, `wiki/mv` | held, plus 2 more callers | grep: also `web/instructions/save.rs:92,170` |
| composability | C3 36 warnings on `--no-default-features` | partly checked | saved log is `--all-targets` (17 warnings, matches the report's second number); 36 not re-run; SIM-20 reports 36 independently |
| design_consistency | DC-01 bar does not wrap; 654 px | held | `frame.css:1,6` no wrap; `inventory.json` `overflowX` 294 on 44 pages, 0 on 52 (pop-ups/errors) |
| design_consistency | DC-02 `.path-cell` loses `max-width:0` under 600 px | held | `project_list.css:4,17-18` as cited; inventory `overflowX` 759 on 2 pages |
| design_consistency | DC-04 bare 400 on four pop-up POSTs | held (read) | same as CW-5 above |
| accessibility | A11Y-01 `/` opens palette unless editable | held | `search.js:225-230` |
| accessibility | A11Y-04 footer links 3 px margin, .875rem | held | `site_footer.css:16` |
| accessibility | A11Y-05 `--muted` on tint 4.32/4.14 | held | `live_contrast.log` lines 22, 39, 47, 82; `--muted:#6f6b64` in `base.css:18` |
| testing | T1 scheme not compared; topcoat policy 403s unmatched POST | held | `POST /nope` `Origin: https://127.0.0.1:3999` → 404 (passes); `Origin: http://evil.example` → 403; `guard.rs:41-43` compares `rest` only |
| testing | T2 zero `.handle(` in `src/web` | held | grep → 0 |
| testing | T4 `plan()` reads `config().pause_gap_minutes` | held | `push.rs:79` |
| build | F2 `Iter` 57.2 s of 110.9 s, starts 53.8 s after `rusqlite` | held | parsed `cargo-timing.html` `UNIT_DATA`: Iter start 53.8 dur 57.2; sqlite build-script run 26.9+26.1 |
| build | F3 `syn 2` only via `iter-macros` | held | `cargo tree -i syn@2 -e normal --depth 1` → `iter-macros` only |
| build | F1 numbers (7.0 s incr, fat 254.7 s incr, dist-opt-s 12.1 MB) | held | `doc/review_tools/build/results.md` rows baseline / lto-fat-cgu1 / dist-opt-s; binary 26 917 392 B |

**30 checks: 28 held, 1 held with a wrong citation, 1 partly checked (log is the `--all-targets` variant). None did not hold.** Other re-runs today: `Host: LOCALHOST:3999` → 403 (CW-15 held); `GET /nope` with `Host: evil` → 404 (TEST proof's layer-scope note held); `</head><body><title>iter</title>` (DC-16/A11Y-18 held); `/agents` on the shared store 2.6 ms (no `running` records there, consistent with P1's cause).

## 6. Ranked backlog (Blocker/High/Medium), in fix batches

Ordering inside a batch is the order to do them. "Test" = what proves it. Batch A′ (correctness) is added to the orchestrator's six because the plan had no home for correctness.

### Batch A — security small (S, ~1 day)
Files: `src/web/mod.rs` (layers), `src/web/guard.rs`, `src/models/organization.rs`, `src/web/platform_pick.rs`, `src/platform.rs`, `src/web/platform.js`, `src/web/ui/prose.rs`, `src/web/ui/notice.rs`, `src/tmux.rs`, `src/args/internal.rs`, `src/agent/exec.rs`, `src/process.rs`.
1. **S-01** headers layer + `Sec-Fetch-Dest` refusal — *test*: T-01 harness asserts headers on `/` and 403 on a framed POST; curl `-sI`; re-load `doc/review_tools/clickjack_poc.html` and see the frame refused. Risk: the `notice.rs:43` inline script needs a nonce or moves to `modal.js` before `default-src 'self'` lands; `search.js` preview iframe needs `'self'`, not `DENY`.
2. **S-02** `http::Uri` parse + re-check on read — *test*: extend the `Organization::check` unit test with the three URLs; `platform.rs` test for a stored bad row. Risk: none.
3. **S-04** platform-activities → POST — *test*: guard test with cross-site GET → 403. Risk: `platform.js` fetch must send the form body.
4. **S-07** `safe_url` rejects `//`, images same-origin — *test*: `prose.rs` unit test. Risk: external images in descriptions stop rendering (intended).
5. **S-08 / C2** `--` in `hook_action` + `allow_hyphen_values` — *test*: `tmux.rs` `FakeRunner` test on the emitted hook string; CLI test `internal hook session-closed -x`. Risk: none; ignored real-tmux test covers quoting.
6. **S-05 / S-06** `env_remove`, prompt on stdin — *test*: `agent/run.rs` command test asserts no `-p <prompt>`; needs the "could not assess" item (does current `claude -p` read stdin?). Risk: medium, verify manually once.
7. **S-12** scheme + Host case — *test*: part of T-01.
Independent of every other batch. S-01 and S-02 can ship alone.

### Batch A′ — correctness small (S–M, ~2 days)
Files: `src/scaffold.rs`, `src/commands/project.rs`, `src/web/org_share.rs`, `src/push.rs`, `src/web/task_runs.rs`, `src/web/task_sessions.rs`, `src/commands/task.rs`, `src/commands/sync.rs`, `src/tmux.rs`, `src/platform.rs`, `src/web/ui/modal.js`, `src/git.rs`/`src/utils/workspace.rs`, `src/wiki/ingest_job/lock.rs`, `src/web/forms/task.rs`.
1. **C-01** refuse dest inside src — *test*: `scaffold` unit test with a nested dest. Risk: none.
2. **C-02** honour `already_pushed`, 303 after push/copy — *test*: `org_share` test with the fake platform (`doc/review_tools/correctness_web/fake_platform.py` pattern) asserting one ledger row and a 303. Risk: the Report page must read `?pushed=` to show the outcome. Pairs with X-03 (`push::prepare`) if done in the same week.
3. **C-03** task layer in `world()` — *test*: `task_runs` test creating `tasks/<id>/instructions/x.md` and POSTing. Risk: none.
4. **C-13** `forms::text` on prompt/message — *test*: CRLF test like `forms/board.rs:63`.
5. **C-06** trim CLI task names, `require_name` on pulled titles — *test*: `commands/task` + `sync` unit tests.
6. **C-09** `exact()` targets — *test*: `FakeRunner` asserts `=name`.
7. **C-11** require `results` key; parse dates — *test*: `platform.rs` tests (one existing test enshrines `{}` → change it deliberately).
8. **C-14** clear hidden instruction on fetch failure — *test*: jsdom test (T-07 runner).
9. **C-10** restore echo on SIGINT; **C-04** check unmerged commits before `branch -D` (M) — *test*: `git` fake; manual once.
10. **C-12** row version on the task edit form (M) — *test*: `edit.rs` fake-form test (T-04). Risk: changes `EditForm` contract; do after T-04 exists.
11. **C-05** `flock` for the wiki lock (M); **C-07/C-08** midnight split and UTC storage (M, design decision first) — *test*: T-06 cases must exist before, they are the proof.
Items 1–9 independent; 10 after T-04; 11 after T-06. Nothing here depends on batch A.

### Batch B — performance small (S–M, ~1.5 days)
Files: `src/agent/run.rs`, `src/agent/solve.rs`, `src/web/task_runs.rs`, `src/web/agents.rs`, `src/web/search_index.rs`, `src/notes.rs`, `src/web/instructions/mod.rs`, `src/wiki/ingest_job/mod.rs`, `src/wiki/ingest_job/review.rs`, `src/web/task_wiki.rs`, `src/agent/task_prompt.rs`.
1. **S-03 + P-01** validate pid, then one `ps` per render — *test*: `FakeRunner` tests for `alive`/`interrupt` incl. `pid 0`, `pid 1`, foreign pgid (T-10); `fsload.sh` + `curl -w '%{time_total}' /agents` < 100 ms at scale 16. Risk: the record-relocation half (out of `--add-dir`) changes `TaskDir::runs_dir`; do validation first, relocation as its own commit. **Must be sequenced with batch A? No — independent, but S-03 is security-rated, so ship with A if A slips.**
2. **P-04** mode before build — *test*: existing `ingest_job` tests; `wiki_bench.sh` `session new` floor.
3. **P-03** orgs once, skip no-notes projects, count not parse — *test*: `routes.sh` `/instructions` at scale 16 < 10 ms.
4. **P-02** cached search entries behind `data_version` + mtimes (M) — *test*: unit test that a new note/task appears after invalidation; `/search` < 20 ms at 6776 notes. Risk: staleness window; keep it ≤ 2 s.
5. **P-05** one `git` probe per render, `waiting` without the store fill (M) — *test*: `wiki_bench.sh` `/task/1?tab=wiki` at 2000 pages.
Independent of each other except 1 (two commits). P-06 (Low) rides on X-16 in batch C.

### Batch C — simplification/structure (M, ~3 days, in SIM's groups A→B→C→D)
Files: `src/db/groups.rs`, `src/md_edit.rs`, `src/wiki/frontmatter.rs`, `src/agent/instruction.rs`, `src/notes.rs`, `src/process.rs`, `src/tmux.rs`, `src/push.rs`, `src/commands/organization.rs`, `src/web/org_share.rs`, `src/web/forms/task.rs`, `src/commands/task.rs`, `src/web/agents.rs`, `src/web/task_runs.rs`, `src/app.rs`, `src/utils/fs.rs` (new), `src/config.rs`, `src/models/project.rs`.
1. **X-01** `Db::org_of` (Medium: removes the error-swallowing variant) — *test*: `cargo test` both feature sets; new test that a DB error propagates.
2. **X-02** `md_edit::split_front` (Medium: one rule) — *test*: existing `instruction`/`notes`/`frontmatter`/`plan` tests stay green (they pin the four rules; expect to adjust one deliberately).
3. **X-03** `push::prepare` (halves platform round-trips; where S-02's re-check lives) — *test*: `push.rs` fake-platform tests count calls. After C-02.
4. **X-04** `Db::move_task`, `secret::rename_for` — *test*: `forms/task.rs`, `commands/task` tests.
5. **X-05** one compose component — *test*: render both pages; a11y `extra.js` on the New agent dialog. Lands W-08/W-10 for that form. Before CO-C8.
6. **X-09** `utils/fs.rs::write_atomic`, `absolute_path` out of `validate` (S) and **X-16** `App::over(db, now)` (S; enables P-06 and T-03) — *test*: `modgraph.py` SCC count drops; `cargo test`.
7. Lows if touching anyway: X-10 small cycles, X-07 `Runner`/`Multiplexer` (enables tests for S-08/C-09), X-12 dead CSS, X-14 wrappers.
Sequence: 1–2 first (core helpers), 3 after C-02, 5 before any `task_runs.rs` move. Independent of A/A′/B except 3.

### Batch D — web consistency and accessibility (S–M, ~3 days)
Files: `src/web/ui/frame.css`, `ui/project_list.css`, `ui/table.css`, `ui/site_footer.css`, `ui/search.css`, `ui/combobox.css`, `ui/drop_zone.css`, `ui/day_nav.rs`, `ui/task_card.css`, `ui/search.js`, `ui/search.rs`, `ui/modal.rs`, `ui/form.rs`, `ui/pick_bar.rs`, `errors.rs`, `task_notes.rs`, `task_sessions.rs`, `task_runs.rs`, `instructions/{save,page,list}.rs`, `task_new.rs`, `org.rs`, `org_report.rs`, `org_share.rs`, `agents.rs`, `sections.rs`, `daily_notes.rs`, `boards.rs`, `project.rs`, `task_agent.rs`, `base.css`.
1. **W-02 + W-03** reflow — *test*: `node doc/review_tools/design_consistency/overflow.mjs <url>` reports nothing; `accessibility/reflow.js` `sw == vw` at 320 on all 47 pages. Risk: `--bar-h` feeds `scroll-padding-top` and the sticky offsets; re-check the agenda's sticky head and `obscured.js`.
2. **W-04** pop-up refusals re-render with `FormError` + `required` — *test*: `accessibility/forms.js` shows title "Error: …", focused `#form-error`; T-02 harness pins the new POST contract. Risk: decide first whether `error_pages` should also dress POST errors (TEST's T2 wants the rule pinned; today "POST stays text" is documented in `web_final_review.md` A6b).
3. **W-05, W-06, W-07, W-21, W-29** CSS-only fixes — *test*: `axe_run.js` 0 `target-size`/`color-contrast`; `live_contrast.js` ≥ 4.5; `keyboard.js` B shows a ring on the card and on the date button.
4. **W-01** `/` toggle — *test*: `keyboard.js` E with the flag set. Blocker by rubric; placed here because it is a settings toggle.
5. **W-08 + W-09** one dismiss idiom and `#new-x` on close URLs — *test*: `keyboard.js` D `active` is the add row; DC inventory lists one exit shape per pop-up. After X-05 for the compose form.
6. **W-10, W-11, W-12, W-16, W-15, W-13** labels, share toggle, tab names, kicker rule, error pages through `frame`, empty-state rule — *test*: `shoot.mjs` + `inventory.json` diff (primary labels set, kicker per page, `filterBar`, footer on error pages); `audit.json` one `aria-label` per tab row. Risk: W-16 changes crumbs on org pages (screenshots differ by design).
7. **W-18, W-19, W-20** palette roles, span arrows, double `aria-current` — *test*: `axe_run.js` 0 `nested-interactive`/`aria-prohibited-attr`; `audit.json` one `aria-current=page`.
8. **W-14** filter bars on `/agents`, `/instructions`; **W-17** slimmer New task pop-up (M) — *test*: screenshots; `inventory.json` `filterBar:true`.
Sequence: 1 and 3 first (pure CSS, independent); 2 before T-02; 5 after X-05. Everything else independent.

### Batch E — build and size (S, ~0.5 day + one 4-minute dist build)
Files: `Cargo.toml`, `macros/Cargo.toml`, `src/agent/instruction.rs` and the 9 other `cfg` sites (X-06), `src/agent/error.rs`.
1. **B-01** `[profile.dist]` block from the build report, keep `release` incremental — *test*: `cargo build --profile dist` → ~12 MB; `cargo test --profile dist` green once (not yet run on `-Os`). Risk: stripped backtraces are addresses; document `split-debuginfo` route.
2. **B-03** `syn = "3"`; **B-04** `[profile.dev]` — *test*: `cargo check -p iter-macros`; `cargo tree -i syn@2` empty; debug clean time.
3. **X-06** `cfg(feature="web")` gates — *test*: `cargo check --no-default-features` 0 warnings (add a CI job with `-D warnings`); CLI binary size recorded before/after.
4. **D-05** fix the `Cargo.toml` comment numbers.
Independent of everything; 3 overlaps file-wise with batch C item 2 (`agent/instruction.rs`), so do 3 after C-2 or before, not during.

### Batch F — tests (S–M, ~3 days)
Files: `src/web/guard.rs` tests, new `src/web/tests.rs` harness, `src/web/edit.rs`, `src/web/drop.rs`, `src/db/tests.rs`, `src/utils/clock.rs`, `src/reporting/tests.rs`, `src/push.rs` tests, `src/fixtures.rs`, `src/agent/run.rs`, the 8 JS files, `doc/review_tools/testing/js/run.sh` → `tests/js/`.
1. **T-01** in-process guard test (`OriginPolicy::dangerous_disable()`, multi-thread runtime) covering the eight header cases + S-01 headers + S-12 — proven feasible; S.
2. **T-03** `XDG_CONFIG_HOME` in fixtures or explicit gap — *test*: run with a deliberately broken real-looking config in a temp home. Before T-02 (same knot: `config()` is a `OnceLock`).
3. **T-02** router harness: `/`→200, `/org/abc`→400 HTML, `/org/999`→404 HTML, POST refusal per the W-04 decision, JSON routes. After W-04 and T-03.
4. **T-06** midnight/DST/leap cases (prove C-07/C-08); **T-05** migration resume from version 3 (prove C-22's comment fix).
5. **T-04** `edit::settle` + `drop_on` tests (prove C-12 before it changes `EditForm`).
6. **T-10** `alive`/`interrupt` via `FakeRunner` (prove S-03/P-01).
7. **T-07** export JS helpers, adopt the jsdom runner (prove C-14, W-09, W-18).
8. Lows: T-08 escaping lock-in, T-09 merge duplicates, T-11 unicode, T-12 clocks.
Sequence: 1 → 2 → 3; 4–7 independent but each should land *before* the fix it proves (A′ 11, A′ 10, B 1, A′ 8).

**Independence summary:** A, A′ (items 1–9), B (2–5), D (1, 3, 4, 6, 7, 8), E (1, 2, 4) can all start today in parallel. Sequenced: B-1 validation→probe; C-3 after A′-2; C-5 before CO-C8 and before D-5; D-2 before F-3; F-2 before F-3; F-4/5/6 before A′-11/A′-10/B-1 respectively; E-3 not concurrently with C-2.

## 7. Stale documentation

| File:line | Says | Truth | Found by |
|---|---|---|---|
| `doc/web_server.md:34` | New pop-up POSTs "re-render the pop-up with the typed values and the error on refusal" | True for New task/New project validation; notes, sessions, runs, instructions answer bare `text/plain` 400 (W-04) | CW-5, DC-04, A11Y-07 |
| `doc/web_server.md` guard section (`:44-53`) | Describes `guard.rs` as the CSRF defence | Also true, but topcoat's `OriginPolicy` runs first and already denies cross-origin non-GET; neither compares scheme; only the guard checks Host. Add a paragraph (conflict #4) | T-O1 |
| `doc/web.md:74` | "A refused form reads as capitalised sentences with a period" | The four bare-400 routes are lowercase fragments without a period; `org_share` shows `unknown format 'md'` raw | DC-04 |
| `doc/web.md:4,14,16-18,22,30,36-38,40` | "`/style.css` were removed"; an "Edit button" and edit pages with crumb "… / Edit", h1 "Edit organization"; task page "Left: description, then Sessions table" | `/style.css` *is* the current sheet (from `base.css`); edit is a "Settings" tab with the entity name as h1 and title "Settings - org-1"; the task page has six tabs | DC-15 |
| `doc/web_final_review.md:24` (A6b) and `:57,59,91,116` | Error pages verified with `/v2.css`; `/v2.css` timings; "unmatched `/nope` keeps topcoat's bare 404" | Sheet is `/style.css`; `/nope` now gets the custom page (fixed since Oct L5); A6b's "POST keeps raw 400/404" is still true and is finding W-04 | DC-15, DC-10, A11Y-21 |
| `design/README.md:8-10`, `design/04-direction-for-iter.md:3-5,14,54,163` | Stylesheet is `/v2.css` from `src/web/v2.css`; tokens scoped on `.v2` | `/style.css` from `base.css` (`ui/mod.rs:124`); tokens on `:root` | DC-15 |
| `doc/web_components.md:113-114` | "no horizontal scroll at 320px" | Every framed page is 654 px wide at 320 px since `0a3f175` (W-02); projects table 1119 px (W-03) | A11Y-02/03, DC-01/02, BP |
| `README.md:661-664` | "pre-commit hook in `.githooks/`"; `git config core.hooksPath .githooks` | No `.githooks` directory exists in the repo | reconciler (`ls .githooks` → not found) |
| `Cargo.toml:46-48` | incremental saves "75s -> 2-27s" | Measured 2026-11-02: rebuild 61.6 s without, 7.0 s with; clean build within noise | F7 |
| `src/db/migrate.rs:187-190` (doc comment) | An index "appears on the first run the schema can take it" | `create_indexes` runs only from `baseline`; a later index needs its own step | C19 |
| `src/app.rs:1-4` | "no config field: config() is already global" | True, and the reason 13 modules read a global and 3 pages hand-build `App` (X-16); note it as a decision or change it | CO-C12 |
| `doc/` (absent) | — | Nothing says that any process running as the user can read the keychain key via `security find-generic-password -w` without a prompt (SEC out-of-scope); users may assume the keychain hides the key from a local agent | SEC |

## 8. What nobody assessed (union of "could not assess")

- A real `claude` run: the generated argv, whether current `claude -p` reads the prompt from stdin (needed for S-06), `begin`'s failure recording, `/agent/{id}/{n}` and plan-step pages (no seeded runs; starting one launches an agent). Design, a11y, security and both correctness lenses all skipped it.
- `Keychain::set`/`delete` against the real keychain; the `connect_real` path (everything ran with `ITER_PLATFORM_KEY=dummy`).
- A real time platform and `gh`: `task pull`/`push`, `org push` network paths (read only; C-11, C-16 are reads), the share push past the preview with a real listing.
- Linux: `/tmp` sharing (S-10), `xdg-open`, umask conventions, system sqlite / native-tls builds; Windows not at all.
- Concurrency: genuinely concurrent requests on one SQLite file (`busy_timeout`, WAL), CLI vs `serve` on the same store (C-05, C-15 are reads), `block_in_place` under many `/search` requests.
- An actual DST transition (C-08 reasoned, not observed) and the `wiki/mv.rs:93` BOM claim.
- topcoat internals: header size limits, request smuggling, slowloris; whether Safari sends `Sec-Fetch-Dest` for framed form posts (the CSP half of S-01 does not depend on it).
- Browser-side behaviour beyond Chrome 156: Safari and Firefox (`inert`, `showPicker`, `clip-path` focus, date inputs); real screen readers (VoiceOver/NVDA/JAWS); HTML5 drag-and-drop under automation (pick mode was exercised instead); real touch targets; 1.4.12 text spacing, 200 % zoom, print, Windows High Contrast beyond emulation; hover-only states and reduced-motion/forced-colours beyond Chrome emulation (design lens).
- `osascript`/`zenity` folder picker (`folder_picker.rs`, `POST /folders/pick`).
- Browser render cost of the 74–346 KB pages; exact rusqlite open cost (approximated with Python sqlite3); runtime effect of `opt-level = "s"/"z"` on `iter serve` and SPARQL.
- `cargo udeps` on the default (web) feature set: nightly 1.91 < topcoat's `rust-version = 1.98` (build and simplification both blocked); unused features; `cargo llvm-lines`/`-Z self-profile` per-view codegen share.
- Line/branch coverage (no tool run); test bodies of `sync.rs`, `platform.rs`, `wiki/graph.rs` beyond names; whether a developer's real `~/.config/iter/iter.yaml` currently changes any test result (never read, per brief).
- Edges created only through trait-object dispatch or macro expansion the composability regex cannot see; whether `recursion_limit` can drop after the cycle fixes; runtime equivalence of any proposed refactor (nothing was modified).
- The seed's `read_dir+filter+sort ×4`, oxigraph package count (74 vs 29), ureq/ring count and binary growth since September: no lens re-measured them.
- Whether the Share modality toggle (W-11) is clearer with a pointer than in a still; the "44px touch rule" on a device.
