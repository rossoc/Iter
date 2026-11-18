## Seed findings from exploration (to be confirmed by reviewers, not final)

Security
- `src/models/organization.rs:63-82` `Organization::check` accepts `http://localhost:1@evil.com/` (rest starts with `:`), so the bearer API key could go over plain http to a foreign host. Check also runs only on write; `platform.rs:147` does not re-check stored rows. Confirm ureq's `RedirectAuthHeaders` default.
- No `X-Frame-Options` / CSP `frame-ancestors` / `nosniff` / `Referrer-Policy` anywhere; fixed port 3000; guard.rs allows same-origin POSTs, so a framed page can click-jack "Run now" (claude with `acceptEdits`), wiki Accept/Revert, push, settings save. `search.js:32` frames own pages so `SAMEORIGIN` is needed.
- `GET /org/{id}/platform-activities` (`platform_pick.rs:89-111`) makes an outbound call with the key on a GET, reachable cross-site (blind).
- tmux hook (`tmux.rs:290`, `commands/session.rs:197`) passes names positionally without `--`; a name starting with `-` is parsed as a flag.
- Agent runs get `--add-dir <taskdir>` + `acceptEdits` and can rewrite `runs/<n>.yaml` (`pid`, `status`); cancel does `kill -TERM -- -<pid>` (`agent/run.rs:216`), `pid: 0` would signal iter's own group.
- `process.rs:19 failure_message` echoes full argv (gh bodies) into errors and the `pushes.results` ledger (`push.rs:398`).
- `commands/project.rs:96-102` + `scaffold.rs:26`: clone copies `.iter-worktrees/` and recurses forever if the target is inside the source.
- Raw error strings on pages: `task_notes.rs:71,85`, `org_share.rs:243`. Markdown allows `//host` links and remote images (`ui/prose.rs:27-38`).
- `task_runs.rs:410,473` validate instruction layers without the task layer the form offers (`task_agent.rs:110`).

Performance
- Every page opens the DB twice (`web/mod.rs:78` + `footer.rs:59`), each open runs PRAGMAs, WAL and `migrate()`.
- `/search` rebuilds the whole index and reads every notes folder per keystroke (`search_index.rs:156`).
- Wiki graph rebuilt per call: `scope::wiki` (`scope.rs:170`) on every `iter wiki` cmd, `ingest::auto` (session new / task done / solve begin) builds before checking `mode`, and the task-runs web page rebuilds it per render (`task_runs.rs:118`, `task_prompt.rs:27`).
- `RunRecord::shown_status` spawns `kill`+`ps` per running run, twice (`run.rs:166-213`, `solve.rs:203,244`).
- `push::plan` N+1 (`push.rs:97-100`), `restore_cmd` N+1 (`commands/misc.rs:91`), `notes::list` reads every file to list (`notes.rs:115`), worklog O(days×tasks×sessions) with clones (`reporting/worklog.rs:54-68`).

Simplification / DRY
- "org of a project" lookup copy-pasted ~7× (`scope.rs:28`, `taskdir.rs:128,143`, `task_prompt.rs:71`, `instruction.rs:36`, `commands/notes.rs:20`, `ingest_job/bridge.rs:24`).
- Two runner abstractions (`process::Tool` vs `process::Runner`) plus free fns; front-matter splitting in 4 places; `read_dir+filter+sort` ×4; editor temp-file code ×2 (`md_edit.rs:65,109`).
- Web: 30 `register` fns + 10 four-line asset routes (one table would do); JSON header code ×4; agent compose form ×2 (`agents.rs:255`, `task_runs.rs:281`); 4 edit modules with the same skeleton; URLs built outside `url.rs` in 9 places; dead CSS (`steps.css` `.plan-progress/.rail-sum/.run-page/.prompt-form/.prompt-from`, `org.css .share-check`, all of `tasks_tab.css`); "dim current on hover" rule ×3; JS date-apply logic duplicated (`board.js:21-47`, `org.js:4-34`), fetch-chain ×3.
- Functions >80 lines: `commands/wiki.rs:36` (167), `wiki/check.rs:94` (161), `push.rs:305`, `commands/session.rs:40`, `wiki/mv.rs:111`, `commands/task.rs:32`, `commands/organization.rs:163`.

Design consistency (web)
- Close/cancel idiom differs in 5 places ("Cancel" `a.u`, "Close" `a.button`, "Back" `a.button`, `.step-close`, `a.button small`) — H1 of the Oct review, still open.
- POST refusals for notes/sessions/runs/instructions/new-task answer bare-text 400 (`task_notes.rs:94`, `task_sessions.rs:81`, `task_runs.rs:410,473`, `instructions/save.rs:65-90`, `task_new.rs:177,217`), contradicting `doc/web_server.md`; messages skip `FormError::sentence`. `org_share` POST renders instead of redirecting.
- ~35 empty-state wordings; 8 hand-written `<p class="empty-line">`; hand-written `class="button"` in 8 files; `/agents` and `/instructions` have no filter bar; `errors.rs` page lacks nav/search/footer; `doc/web_final_review.md` A6b and `/v2.css` references are stale.

Build / size (facts)
- 297 lock packages; 223 crates with defaults, 147 with `--no-default-features`. topcoat pulls 114 (incl. tungstenite→sha1 0.10 and h2 despite `default-features=false`), oxigraph 74 (zerocopy 15 MB rlib), ureq 29 (ring), clap_complete `unstable-dynamic` 22. Duplicates: syn 2/3 (three rlibs 6.5–9.6 MB), sha1/digest/block-buffer, base64, getrandom, hashbrown, rand.
- Release binary 25.7 MB unstripped (`__LINKEDIT` 8 MB, 57k symbols), grew 4 MB (Sep) → 26.9 MB. Profile: only `incremental = true`; no `.cargo/config.toml`, no `--timings` ever run. topcoat uses `catch_unwind`, so `panic = "abort"` is unsafe for the server.

