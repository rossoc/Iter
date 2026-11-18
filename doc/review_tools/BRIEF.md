# Review brief (2026-11-02) — read fully before doing anything

Repo (git worktree, branch `feat/review`): `/Users/carlorosso/.config/iter/.iter-worktrees/review`.
Run every command from there. Do NOT `cd` to the parent `/Users/carlorosso/.config/iter`: that is the
user's real config directory and holds the live database `iter.db`. Never read or write it.

## Hard rules
- Source is READ-ONLY. You edit nothing under `src/`, `macros/`, `Cargo.*`. The one exception is the
  build-time agent, which works in a `git worktree`-free copy under its scratch dir (see its prompt).
- You write exactly one report, `doc/review_2026-11-02_<lens>.md`, and scripts under
  `doc/review_tools/<lens>/`. Do not read other agents' 2026-11-02 reports. You MAY read the older
  `doc/review_*.md`, `doc/web_final_review.md`, `doc/web_cleanup.md` (dated 2026-10-01) to avoid
  repeating finished work; mark anything still open from them as "re-confirmed" or "fixed since".
- Every finding cites `file:line` and is tagged with its evidence: `run` (you executed something and
  saw it), `read` (you read the code), `inferred` (neither). Blocker/High needs `run` or a very
  precise `read`; else downgrade.
- Never touch `~/.config/iter`. Every `iter` or `cargo test` invocation carries the environment below.
- Treat the "seed findings" as unconfirmed hypotheses: confirm, refute or ignore them with evidence.

## Environment
```
export ROOT=/Users/carlorosso/.config/iter/.iter-worktrees/review
export CARGO_TARGET_DIR=$ROOT/target            # prebuilt release + test artifacts live here
export BIN=$ROOT/target/release/Iter             # release binary of HEAD
export SCRATCH=/private/tmp/claude-501/-Users-carlorosso--config-iter--iter-worktrees-review/f9634ce8-ea6d-484c-8703-6013688875b7/scratchpad
export XDG_CONFIG_HOME=$SCRATCH/iter-home        # the shared seeded store (scale 4), see below
export ITER_PLATFORM_KEY=dummy                   # keeps the macOS keychain out of it
unset TMUX
```
- `cargo test` (release artifacts are not reused by tests; a debug test build exists after Phase 0):
  `XDG_CONFIG_HOME=$SCRATCH/test-home cargo test 2>&1 | tail` (web feature on by default;
  `--no-default-features` for CLI only). ~545 tests, 3 ignored.
- A shared server is running on the scale-4 store: `http://127.0.0.1:3999` (started by the
  orchestrator with `$BIN serve -p 3999 --no-open`). Browser-driven lenses use it. If it is down,
  start your own on another port against your own store.
- Your own store at any size: `doc/review_tools/seed.sh <dir> <scale> $BIN` then
  `XDG_CONFIG_HOME=<dir> $BIN serve -p <port> --no-open`. Scale 1 = 2 orgs / 5 projects / 30 tasks /
  200 sessions; scale N multiplies. Use ports 4000-4099, one per agent (performance 4001-4003,
  design 4010, accessibility 4020, security 4030, testing 4040). Empty store: just an empty dir.
- Browser: headless Chrome at `/Applications/Google Chrome.app/Contents/MacOS/Google Chrome`
  (`--headless=new --screenshot=... --window-size=1280,900 <url>`; `--force-dark-mode` or
  `--enable-features=WebContentsForceDark` for dark, or emulate `prefers-color-scheme` via CDP).
  `npx` works but no Playwright/axe is installed; `npx -y @axe-core/cli` or `npx -y playwright`
  will download on first use (allowed, into the npm cache). Put any node scripts under your
  `doc/review_tools/<lens>/` folder and run them with absolute paths.
- `sqlite3` is at `/usr/bin/sqlite3` (use it for `EXPLAIN QUERY PLAN`). `cargo udeps` is installed;
  `cargo bloat` is not.

## Route map (from doc/web_server.md)
`/`, `/settings`, `/org/{id}` (`?tab=info|tasks|report`, `?share=1`), `/org/{id}/edit`,
`/org/{id}/notes/{date}`, `/org/{id}/platform-activities` (JSON), `/project/{id}` (`?tab=`),
`/project/{id}/edit`, `/project/{id}/task/new`, `/project/{id}/notes/{date}`, `/task/{id}` (tabs
info/sessions/notes/agent/wiki...), `/task/{id}/edit`, `POST /task/new`, `/agents`, `/agent/{id}/{n}`,
`/boards`, `/board/{id}` (agenda, `?date=`, `?pick=`), `/board/{id}/matrix`, `/board/{id}/info`,
`/board/{id}/edit`, `POST /board/{id}/{schedule,matrix}`, `/instructions`, `/instruction/{name}`,
`GET /search?q=` (JSON), `POST /folders/pick`, `/style.css`, `/board.js`, `/org.js`, `/platform.js`,
`/favicon.ico`. Any `{id}` that is not a number is a 400; unknown id is a 404.

## Finding schema
`ID · lens · severity · file:line · evidence(run|read|inferred) · effort(S/M/L) · one-line fix`
followed by 1-5 lines of explanation and the exact repro (command, URL, test name) when `run`.

## Severity rubric
- **Blocker**: broken feature, data loss, security issue exploitable from a web page, fails WCAG A.
- **High**: fails WCAG AA; a bug likely to bite on the next edit with user impact; secret exposure
  needing a precondition; O(n²) on a path users hit; >10% of a module is duplicated logic.
- **Medium**: narrower failure or clear degradation; inconsistency a user notices.
- **Low**: tidy-up or latent. Cosmetic maintenance findings are Low however structural they sound.

## Report skeleton
Summary (half page, top 5) · Scope · Method (commands, tools, files read, what was NOT checked) ·
Findings (table + details) · Confidence · Out-of-scope observations (incl. security observations,
always) · What I could not assess.

## Seed findings (unconfirmed; from a read-only exploration pass)
See `doc/review_tools/SEED_FINDINGS.md`.
