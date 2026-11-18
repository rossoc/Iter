# Review 2026-11-02 — simplification

Lens: where the code can be made smaller or simpler without changing behaviour. Branch `feat/review`
at `5d96f44`, worktree `/Users/carlorosso/.config/iter/.iter-worktrees/review`. Source untouched;
scripts under `doc/review_tools/simplification/`.

## Summary

The codebase is already well factored: the exact-clone rate is about 0.4% (133 of ~30k normalised
lines, identifiers kept) and default `cargo clippy` is clean (0 warnings). What is left is
*semantic* duplication: the same rule written twice in different words, mostly across the
CLI/web seam and the agent/wiki seam. About **300 lines** can go with no behaviour change (another
~180 if the route table is replaced by topcoat's `discover` feature, at the cost of one crate).

Top 5:
1. **SIM-01** the "organization of a project" lookup is written 8 times in two variants (one swallows
   DB errors, the other propagates them) — one `Db::org_of(&Project)` fixes a latent drift.
2. **SIM-02** front matter is split in 7 places with 4 different rules (BOM handling, `\n---\n`
   search vs line-exact divider, open-ended front allowed or not) — one `md_edit::split_front`.
3. **SIM-03** `process::Runner` re-implements `run`/`output_trimmed` that the free functions already
   hold, and `Tool` is a third face of the same five operations; `tmux.rs` adds four free-function
   wrappers over `TmuxClient::system()`.
4. **SIM-06** the push flow (target override → plan → connect → check → warnings → execute) is
   written once for the CLI and once for the web; **SIM-07** the "a task with a session cannot move"
   rule and the "rename the API key with the org" rule likewise.
5. **SIM-12/13** the web: 30 `register` functions + 10 four-line asset routes (≈210 lines of
   plumbing), the JSON response header spelled out 4 times, the agent compose form written twice
   (already drifting: one has a Close button, the other not).

## Scope

Whole repo: `src/` (216 .rs), `src/web/**` (44 .css, 8 .js), `macros/`. Tests read only to
exclude them from clone counts. Older reviews (`doc/review_dry.md`, `doc/review_complexity.md`,
`doc/review_composability.md`, `doc/web_cleanup.md`, `doc/web_final_review.md`, 2026-10-01) read to
mark finished work.

## Method

Commands (all from the worktree, `XDG_CONFIG_HOME=$SCRATCH/test-home`, separate target dirs):

- `python3 -I doc/review_tools/simplification/clones.py --window 6 --ids keep src macros` and
  `--ids mask`; `--ext .js,.css --window 4 src`. Normalised 6-line token windows (comments,
  whitespace stripped; string/number literals masked; `mod`/`use` lines and `#[cfg(test)]` tails
  skipped; periodic self-overlaps collapsed). Output kept in the scratchpad.
- `python3 -I doc/review_tools/simplification/css_dead_classes.py src/web src` — every class
  selector in the sheets vs. word-use in all `.rs`/`.js` (first run scanned only `src/web` and
  wrongly flagged `.lost`, which `src/agent/run.rs:171` emits; fixed, rerun).
- `python3 -I doc/review_tools/simplification/fn_lengths.py 80 src macros` — brace-matched
  function lengths.
- `python3 -I doc/review_tools/simplification/unused_pub.py src macros` — `pub` items whose name
  occurs once outside tests (heuristic).
- `cargo +nightly udeps --all-targets`: **refused** — nightly is 1.91 and `topcoat 0.9` requires
  rustc 1.98. `cargo +nightly udeps --no-default-features` (CLI only): "All deps seem to have been
  used." The web dependency set could not be checked.
- `cargo check --all-targets` (default features): 0 warnings. `cargo +nightly check
  --no-default-features`: 36 dead-code warnings (web-only code in the core, see SIM-20).
- `cargo clippy --all-targets`: 0 warnings. With `-W clippy::pedantic` (noisy lints allowed): 121,
  of which 34 `format_push_string`, 35 `match_bool`, 16 `float_cmp`; listed under SIM-22.
- Read in full or in the cited ranges: every file named below.

NOT checked: runtime behaviour (no server started; no page rendered); `macros/src/lib.rs` beyond
its length; whether topcoat's `discover` feature compiles with `default-features = false`
(inferred from its `Cargo.toml`); the `.js` files beyond the cited ranges.

### Clone detector result (run)

| mode | pairs | normalised lines in pairs | share of ~30k |
|---|---|---|---|
| identifiers kept, 6-line windows | 12 | 133 | 0.4% |
| identifiers masked (structural) | 130 | 984 | 3.3% |
| `.js`/`.css` only, 4-line windows | 2 | 9 | — |

Top pairs (identifiers kept):

| lines | A | B | what |
|---|---|---|---|
| 38 | `src/web/ui/mod.rs:62-99` | `:68-105` (period 6) | six asset routes, SIM-13 |
| 24 (masked) | `src/web/mod.rs:92-115` | `:98-121` (period 6) | register table, SIM-12 |
| 18 | `src/web/quadrants.rs:27-48` | `:35-56` (period 8) | `QUADRANTS` data table — fine |
| 14 | `src/models/organization.rs:23-44` | `src/models/project.rs:38-68` | settings fields ×2, SIM-09 |
| 9 | `src/commands/ingest.rs:91-99` | `:149-157` | `Job {..}` built twice, SIM-10 |
| 9 | `src/commands/sync.rs:50-58` | `:215-223` | pull/push prologue, incidental |
| 8 | `src/web/org_edit.rs:20-31` | `src/web/project_edit.rs:19-30` | `use` blocks |
| 6 ×3 | `src/web/ui/field.rs:81,133,157` | | component signatures — fine |

The masked pass adds the four `*_edit.rs` `screen` tails (10 lines ×6 pairs, SIM-16), the
`daily_notes.rs` route pairs (SIM-17), `sections.rs` `Section`/`TaskSection` (two enums with
`of`/`title`, 14 lines; data, fine), and `error.rs`/`agent/error.rs` variant lists (declarations).
Everything else is struct fields or signatures.

## Findings

Schema: `ID · lens · severity · file:line · evidence · effort · one-line fix`, then explanation.
"Lines" is the estimated net removal.

| ID | sev | where | evidence | effort | lines | one-line fix |
|---|---|---|---|---|---|---|
| SIM-01 | Medium | 8 sites, see below | read | S | 24 | `Db::org_of(&Project) -> Result<Option<Organization>>` |
| SIM-02 | Medium | 7 sites, see below | read | S | 25 | one `md_edit::split_front(&str) -> Option<(&str,&str)>` |
| SIM-03 | Medium | `src/process.rs:96-182`, `src/tmux.rs:235-249` | read | M | 47 | `Runner::{run,output_trimmed}` call the free fns; `Tool` = `Git` over `Runner`; drop tmux free fns |
| SIM-04 | Low | `src/md_edit.rs:65-70,109-114`; `src/notes.rs:78-85,167-174` | read | S | 12 | `fn temp_buffer()`; `notes::open_front(content)` |
| SIM-05 | Low | `src/platform.rs:146-167` | read | S | 6 | `fn agent(secs) -> ureq::Agent`, `new(url,key,secs)` |
| SIM-06 | Medium | `src/commands/organization.rs:171-186` vs `src/web/org_share.rs:287-303` | read | M | 15 | `push::prepare(db, org, range, now, overrides, platform) -> (Plan, Check, Vec<String>)` |
| SIM-07 | Medium | `src/commands/task.rs:189-194` vs `src/web/forms/task.rs:133-141`; `src/commands/organization.rs:30-34` vs `src/web/forms/org.rs:64-69` | read | S | 10 | `Db::move_task(id, &mut task, target)`; `secret::rename_with(db, id, old)` |
| SIM-08 | Low | `src/db/pushes.rs`; `src/wiki/graph.rs:279-281`; `src/db/reporting.rs:21-28` | read | S | 16 | fold `find_push` into `db/reporting.rs`; drop `Wiki::node` (alias of `resolve`); drop `sessions_for_project_in_range` |
| SIM-09 | Low | `src/models/organization.rs:23-44` vs `src/models/project.rs:38-68` | read | L | 0 | leave: `Settings` is already the shared type; flattening needs `Table` derive support |
| SIM-10 | Low | `src/commands/ingest.rs:91-99,149-157` | run | S | 9 | `Target::job(&self, scope, now, schema) -> Job` |
| SIM-11 | Low | `src/wiki/scope.rs:125-131` | read | S | 3 | `db.projects_in::<Organization>(o.id())?` instead of `list` + filter |
| SIM-12 | Low | `src/web/mod.rs:91-124` + 30 `register` fns | read | M | 0 / 180 | keep, or enable topcoat `discover` (+`inventory` crate) and delete the 30 fns |
| SIM-13 | Low | `src/web/ui/mod.rs:112-152`, `layout.rs:35-43`, `org.rs:44-50`, `platform_pick.rs:23-31` | run | M | 35 | `impl Route for (&'static str, &'static Asset)` or `Asset::route(path)`; one table |
| SIM-14 | Low | `src/web/folders.rs:36-41`, `task_runs.rs:453-456`, `platform_pick.rs:103-108`, `search_index.rs:162-168` | read | S | 15 | `fn json(cx, body: String) -> Result<Response>` |
| SIM-15 | Medium | `src/web/agents.rs:255-277` vs `src/web/task_runs.rs:281-311` | read | M | 20 | one `compose_form(action, instruction, model, effort, prompt, hint, editing, close)` component |
| SIM-16 | Low | `src/web/{org,project,board,task}_edit.rs` show/save pairs | read | M | 15 | re-confirmed "acceptable" (review_dry F11); residual is the two handlers per module |
| SIM-17 | Low | `src/web/daily_notes.rs:268-312` | run | S | 15 | one generic per pair, `Owner::project`/`Owner::org` as the only variable |
| SIM-18 | Low | `src/web/task_agent.rs:285`; `src/web/url.rs:213-215` | read | S | 4 | `agent_tab_url(&task_url(id))`; delete `run_url` (3 callers → `agent_url`) |
| SIM-19 | Low | 7 CSS classes, 3 hover rules, 3 `:root` files | run | S | 20 | delete dead rules; drop `tasks_tab.css`; leave the hover rules |
| SIM-20 | Low | `src/agent/instruction.rs` (≈300 lines), 9 other items | run | S | 0 | `#[cfg(feature = "web")]` on web-only code the CLI binary still compiles |
| SIM-21 | Low | `src/web/board.js:21-47` vs `org.js:4-31`; fetch chain ×3; debounce ×5 | read | M | 10 | only worth it with a shared `common.js` asset (adds a route) |
| SIM-22 | Low | pedantic clippy sites, see below | run | S | 5 | `write!` instead of `push_str(&format!())` ×34; 2 needless `Result`s |
| SIM-23 | Low | 14 functions >80 lines | run | M | 0 | split by responsibility (list below); no line savings |

Severity: Blocker 0 · High 0 · Medium 6 (SIM-01, 02, 03, 06, 07, 15) · Low 17.
Estimated removable: **≈300 lines** (SIM-01..11, 13..19, 21, 22), **≈480** with SIM-12's discover
option. Nothing here reaches "High": no module has >10% duplicated *logic*; `process.rs` is
closest (≈45 of 246 lines, 18%, but half of it is the test fake, which is legitimate).

### SIM-01 (Medium) The org-of-project lookup, 8 times in 2 variants

`src/wiki/scope.rs:28-33` (`org_of`, propagates), `src/agent/taskdir.rs:128-132` (propagates),
`src/agent/taskdir.rs:141-144` (`.ok().flatten()`, swallows), `src/agent/task_prompt.rs:72-75`
(propagates), `src/agent/instruction.rs:34-37` (swallows), `src/commands/notes.rs:19-22`
(propagates), `src/wiki/ingest_job/bridge.rs:23-26` (propagates), `src/web/instructions/mod.rs:85-87`
(propagates). The seed said 7; it is 8. Two of them turn a DB error into "no organization", which
changes which notes folder is used (`notes::project_dir(project, &None)`). Bug-risk: the two
variants already disagree. Fix: `impl Db { pub fn org_of(&self, p: &Project) -> Result<Option<Organization>> }`
next to `projects_in` in `src/db/groups.rs`; `scope::org_of` becomes a call. Group A.

### SIM-02 (Medium) Front matter split, 7 ways

`src/md_edit.rs:34-37` (strips `---\n`, divider else whole), `src/notes.rs:78-85` and `:167-174`
(BOM, `opened` flag), `src/wiki/frontmatter.rs:33-51` (BOM, closed only), `src/wiki/mv.rs:93-96`
(no BOM), `src/agent/instruction.rs:57-71` (own scanner: `rest.find("\n---\n")` accepts `---` with
trailing spaces that `split_at_divider` rejects, and a trailing `\n---` without newline),
`src/agent/plan.rs:190-200` (`body_start`, counts lines), `src/commands/solve.rs:48-50`. Four
rules for "where does the front matter end". Fix: make `frontmatter::split` (BOM + `---\n` +
divider → `Option<(yaml, body)>`) live in `md_edit` and have the six others call it;
`instruction::split`'s `\n---` without trailing newline case can be kept by trimming before the
call if a test depends on it (check `src/agent/instruction.rs` tests). Group A.

### SIM-03 (Medium) Three faces of "run an external tool"

`src/process.rs:29-94` five free functions; `:99-120` `trait Tool` forwarding to them; `:136-166`
`trait Runner` whose provided `run` (`:145-150`) and `output_trimmed` (`:153-165`) re-implement
`:41-47` and `:73-84` line for line. `SystemRunner` (`:168-180`) forwards the other three. Fix:
`Runner::run`/`output_trimmed` default bodies call the free functions on `self.run_status`/
`self.output` results via two tiny shared helpers (`fn nonzero(tool,args)->IterError`, `fn trimmed(String)->Option<String>`),
−18; `Git` (`src/git.rs`, 6 call sites) could be `struct Git<'a>(Option<&'a str>)` over `&'static dyn Runner`
and `Tool` deleted, −20 (M; keeps the test fake). `src/tmux.rs:235-249`: four free fns over
`TmuxClient::system()` with 6 callers (`utils/clock.rs:105,113`, `utils/resolve.rs:51`,
`utils/workspace.rs:88,102`, `commands/project.rs:208`) — inline `TmuxClient::system().x()` at the
callers, −12. Group B.

### SIM-04 (Low) Editor temp file ×2, notes front-matter prologue ×2

`src/md_edit.rs:65-70` and `:109-114` build the same `iter-edit-{pid}-{stamp}.md` path; one
`fn temp_buffer(text) -> Result<PathBuf>` also shares the `write`. `src/notes.rs:78-85` and
`:167-174` both strip the BOM, strip `---\n`, remember `opened`, call `split_at_divider`; a
`fn open_front(content) -> (bool, Option<(&str,&str)>, &str)`. Group A.

### SIM-05 (Low) `JsonWebhook::with_timeout` repeats the builder

`src/platform.rs:146-152` and `:160-167` are the same four builder lines with a different number;
`with_timeout` has one caller (`src/web/platform_pick.rs:40`). `fn agent(seconds) -> ureq::Agent`
and `new(url, key)` → `agent(30)`. Group B.

### SIM-06 (Medium) Push flow, CLI and web

`src/commands/organization.rs:171-186` and `src/web/org_share.rs:287-303`: `Target::of(org)`,
override organization/activity, `push::plan`, connect, `push::check`, `push::unknown_targets`,
`push::execute`. The CLI tolerates a missing key on `--dry-run`, the web does not; both call
`push::resolve` twice per push (`unknown_targets` at `push.rs:253` and `execute` at `:351` — the
seed's "resolving twice", confirmed, two platform round-trips). Fix: `push::prepare(...)` returning
`(Plan, Check, Resolved, Vec<String>)` once, `execute` taking the `Resolved`. Group C (touches both
binaries' behaviour only in the number of HTTP calls).

### SIM-07 (Medium) Two rules written twice across the seam

- Move-with-session: `src/commands/task.rs:189-194` and `src/web/forms/task.rs:133-141` both say
  "if the target project differs and `find_session_config_by_task` is some → `TaskHasSession`",
  then set `project_id`. The CLI resolves by name, the web by id; the error carries
  `task_display` in one and `task.name` in the other (already drifted). A `Db::move_task(id, &mut Task, &Project)`.
- API-key rename: `src/commands/organization.rs:30-34` and `src/web/forms/org.rs:64-69` both
  re-read the org and `secret::rename_key(old, new)`. One `secret::rename_for(db, id, old_name)`.
- The seed's `utils/crud.rs:47-63` vs `web/forms/org.rs:63` / `board.rs:41`: confirmed both call
  `db.save_group`; the only logic duplicated is the rename above. The task *field* validation
  (`web/forms/task.rs:88-125`) has no CLI twin because the CLI parses typed YAML (`TaskEdit`) —
  refuted as duplication; the web's `BAD_*` prefixes are a web-only concern. Group C.

### SIM-08 (Low) Tiny wrappers

- `src/db/pushes.rs` (13 lines, one 6-line method) → into `db/reporting.rs` or `db/mod.rs` with
  its `mod` line: −7.
- `src/wiki/graph.rs:279-281` `node(from,id)` ≡ `resolve(from,id)`; 9 callers (2 non-test) → −3.
- `src/db/reporting.rs:21-28` `sessions_for_project_in_range` has one caller
  (`src/utils/report.rs:91`) → `sessions_for_projects_in_range(&[id], ..)`: −8.
- `src/models/session_config.rs`: a 14-line model, not a wrapper — refuted.
- `src/db/migrate.rs:44-50`: `add_notes_path` and `add_platform` have identical bodies, but each is
  a shipped migration step (`MIGRATIONS` says "never edit a shipped one") — refuted; leave.
Group B.

### SIM-10 (Low) `Job` built twice — `src/commands/ingest.rs:91-99,149-157` (run: clone pair #5). A
`Target::job(&self, scope: &str, now: &str, schema: &str) -> Job` or a `fn job<'a>(...)`. Group A.

### SIM-11 (Low) `src/wiki/scope.rs:125-131` loads every project and filters by
`organization_id == Some(o.id())`; `Db::projects_in::<Organization>(id)` (`src/db/groups.rs:7`)
is the same set via SQL (`ORDER BY name` — check `levels()` does not rely on insertion order;
`siblings` is pushed in list order and nothing sorts it afterwards, so the order of sibling levels
changes from id to name — behaviour-visible in `iter wiki export`; keep `list` or sort explicitly).
Group B, or skip.

### SIM-12 (Low) 30 `register` functions

`src/web/mod.rs:91-124` lists 30 `x::register`; each module has a 3-5 line `pub fn register` plus a
`RouterBuilder` import (150 lines + 30 imports, `run`). topcoat 0.9 has `RouterBuilder::discover_routes()`
/`discover_pages()` behind its `discover` feature (`inventory` crate), which the `Cargo.toml:34-38`
comment deliberately leaves off to save crates. This is a chosen trade-off, so Low: either keep, or
turn `discover` on and delete the 30 fns (−180, +1 small crate; `inventory` is link-time, so
`cargo udeps` is unaffected). Group D (its own, since it changes the dependency set).

### SIM-13 (Low) 10 four-line asset routes

`src/web/ui/mod.rs:124-152` (6), `src/web/layout.rs:35-43` (2), `src/web/org.rs:46-50`,
`src/web/platform_pick.rs:27-31`: each is `static X: Asset`, `#[route(GET "/x")] async fn x(cx) { X.respond(cx) }`
and a `.route(x)`. `Route` (`topcoat-router-0.9.0/src/route.rs:53`) is a four-method trait (`id`,
`methods`, `path`, `handle`), so a `struct Static(&'static str, &'static Asset)` implementing it
turns the ten into one table `[("/style.css", &CSS), ...]` folded into `builder.route(..)`: ≈25
lines in, ≈60 out. Keep `respond`. Group D.

### SIM-14 (Low) JSON header ×4 — `src/web/folders.rs:36-41`, `src/web/task_runs.rs:453-456`,
`src/web/platform_pick.rs:103-108`, `src/web/search_index.rs:162-168` (the last adds `no-store`).
`fn json(cx: &Cx, body: String, store: bool) -> Result<Response>` in `web/mod.rs` or `assets.rs`.
Group D.

### SIM-15 (Medium) Agent compose form ×2

`src/web/agents.rs:255-277` (edit an existing run) and `src/web/task_runs.rs:281-311` (new run):
same `<datalist id="model-list">`, same `compose-pick` row (`select agent`, `field model`,
`select effort`), same `textarea prompt`, same `compose-actions` buttons. Already drifted: the
hints differ, only one has `Close`, only one has `loaded:`. Bug-risk (a field added to one form
and not the other). One component with `editing`/`close`/`instruction` options, −20. Group D.

### SIM-16 (Low) Four edit modules — review_dry F11 said the `show`/`save`/`screen` skeleton is
acceptable because `EditForm` + `edit_page` carry the logic; re-confirmed. The remaining structural
duplicate is the two handlers per module (`load` + `screen` / `submit` + `screen`,
`src/web/org_edit.rs:29-50`, `project_edit.rs:28-47`, `board_edit.rs:54-76`, `task_edit.rs:43-69`).
A generic needs the `screen` component as a parameter, which topcoat's `#[component]` makes
awkward; −15 at best. Leave unless touching anyway.

### SIM-17 (Low) `src/web/daily_notes.rs:268-312`: six handlers in three identical pairs
(`project_note`/`org_note`, `*_save`, `*_today`) differing only in `Owner::project` vs
`Owner::org`. `fn owner(cx, kind) -> Result<Owner>` or three generic bodies: −15 (run: masked pairs
`daily_notes.rs:268-274 ↔ 277-283`, `283-294 ↔ 291-302`). Group D.

### SIM-18 (Low) URLs outside `url.rs` — re-checked every seed site. `org_share.rs:187,188,405`,
`org_edit.rs:77`, `sections.rs:84,172`, `task_notes.rs:33`, `task_sessions.rs:32` are
`format!("{base}/share")`-style suffixes on a `url::` base: moving them to `url.rs` is a wash in
lines (Low, consistency). The one real hard-coded path is `src/web/task_agent.rs:285`
`format!("/task/{task_id}?tab=agent")` → `agent_tab_url(&task_url(task_id))`. `url.rs:213-215`
`run_url` is `agent_url` under another name (3 vs 9 callers) — delete, −4. Group D.

### SIM-19 (Low) CSS — dead classes, repeated rules, tokens (run)

Dead (0 uses in any `.rs`/`.js`, `css_dead_classes.py`): `.plan-progress` (`src/web/ui/steps.css:1-2`),
`.rail-sum` (`:31,101`), `.run-page` (`:54` part, `:65-67`), `.prompt-form`/`.prompt-from`
(`:70-72`), `.share-check` (`src/web/org.css:43`, shares a rule with `.share-preview`),
`.tab-actions` (`src/web/ui/tasks_tab.css`, the whole 2-line file + its `include_str!` at
`ui/mod.rs:87`). Seed confirmed in full, plus `.lost` refuted (emitted at `src/agent/run.rs:171`).
≈15 lines. The "dim current on hover" rule (`ui/frame.css:13-14`, `org.css:13-14`,
`ui/tabs.css:9-10,16`) is three variants (`::before` transform, `::before` opacity, `::after`
transform) — not one rule; a shared class would save ~4 lines, leave. `:root` tokens: 59 in
`base.css` (4 blocks), 6 in `ui/layout.css`, 1 in `ui/page_header.css` — the seed's "3 files"
confirmed; moving the 7 saves ~3 lines; cosmetic. Group E.

### SIM-20 (Low) Web-only code the CLI binary compiles (run)

`cargo +nightly check --no-default-features` warns 36 times: nearly all of `src/agent/instruction.rs`
(`Front`, `parse_front`, `list`, `find`, `check`, `compose`, `Fields`, `KINDS`, … ≈300 lines, only
`split`/`file`/`global` are used by `agent/prompt.rs`/`task_prompt.rs`), plus
`src/wiki/ingest_job/review.rs:77,87`, `src/wiki/ingest_job/spawn.rs:57 ingest_in_background`,
`src/push.rs:264 choices`, `src/agent/run.rs:41 models/efforts`, `:177 instruction_name`,
`:207 interrupt`, `src/agent/plan.rs:93 phase_progress`, `src/agent/taskdir.rs:140 in_notes`,
`src/platform.rs:160 with_timeout`, variants in `src/agent/error.rs:23`. The repo already uses
`#[cfg_attr(not(feature = "web"), allow(dead_code))]` for this in 8 places
(`reporting/mod.rs:144,184`, `db/groups.rs:155,194`, `db/mod.rs:329`, `secret.rs:81`,
`fixtures.rs:24,85`, `reporting/merge.rs:114`). Not removable lines, but a `#[cfg(feature = "web")]`
on the instruction module's web half shrinks the CLI binary and makes the CLI build warning-free
again. All 12 `allow(...)` sites in the tree are conditional (`not(test)` or `not(feature)`); none
hides unconditional dead code. The `unused_pub.py` heuristic's 12 hits are glob re-exports and
route-macro targets — no real unused `pub`.

### SIM-21 (Low) JavaScript

`src/web/board.js:21-47` and `src/web/org.js:4-31`: the same `ready`/`typing`/`keydown`/`change`/
`focusout` date-apply pattern; they differ (org debounces 250 ms, validates two fields, accepts
""). The `fetch(...).then(r => { if (!r.ok) throw new Error(r.status); return r.json(); })` line is
identical in `platform.js:15`, `ui/modal.js:35`, `ui/path_pick.js:19` (and a variant at
`ui/search.js:58-60`). `clearTimeout/setTimeout` debounce in `org.js:9`, `platform.js:28`,
`ui/combobox.js:318`, `ui/search.js:145,213`. Each page loads only the scripts it needs and there
is no shared module, so sharing costs a `common.js` asset and a route; net ≈ −10. Leave unless
SIM-13 lands.

### SIM-22 (Low) Hand-rolled vs std/crate — clippy pedantic (run)

Default clippy: 0. Pedantic: 34× `push_str(&format!(..))` → `write!` (`commands/wiki.rs:57,62,65,
85,...`, `wiki/check.rs`, `reporting/*`), 2× needless `Result` wrap (`src/db/sqltypes.rs:38`,
`src/web/org_share.rs:496`), `src/secret.rs:94` unused `self`, `src/agent/run.rs:256` redundant
`continue`, `src/web/blocking.rs:11` needless `async`, 5× `DateRange`/`&self` by reference
(`web/org_report.rs:92,97,134`, `reporting/merge.rs:65,69`). No helper duplicates a std function
(`platform.rs:125 path_segment` is percent-encoding, which std lacks; `project_rows.rs:15 tilde`
is app-specific). ≈5 lines.

### SIM-23 (Low) Functions over 80 lines (run, `fn_lengths.py`)

`src/commands/wiki.rs:36 run` 167 (the text formatters for Grep/Neighbors/Check/Mv are inline
closures while Find/Show have `find_text`/`show_text` — move the four out, −0 lines, +readability),
`src/wiki/check.rs:94 check` 161 (five independent lint passes in one body: per-node, dangling,
unplaced, dirty, promote — five fns pushing into one `Vec`), `macros/src/lib.rs:30 expand` 134,
`src/push.rs:305 execute` 114 (platform half / GitHub half / ledger), `src/commands/session.rs:40
session_new` 113 (the stale-row branch `:49-77` is its own fn), `src/wiki/mv.rs:111 mv` 108
(validate / plan / apply), `src/commands/task.rs:32 run` 98 (a dispatch `match`, fine),
`src/commands/organization.rs:163 org_push` 92 (SIM-06), `src/web/org_edit.rs:53 screen` 86,
`src/web/instructions/page.rs:13 edit_form` 83, `src/web/task_runs.rs:240 compose_modal` 82
(SIM-15), `src/web/search_index.rs:33 entries` 82, `src/wiki/retrieve.rs:65 find` 81,
`src/agent/solve.rs:199 begin` 81. 29 functions are over 60 lines. The seed's list is confirmed
exactly.

### Refuted or already done

- `#[allow(dead_code)]`: none unconditional (SIM-20).
- `cargo udeps`: nothing unused in the CLI set; web set not checkable on this nightly.
- `web/forms/task.rs` vs `commands/task.rs:172` validation: not a duplicate (typed YAML vs form
  strings), SIM-07.
- `migrate.rs:44-50`, `models/session_config.rs`: not wrappers to remove, SIM-08.
- `*_edit.rs` skeleton: factored in Oct (`doc/web_cleanup.md` P3, `doc/review_dry.md` F11), SIM-16.
- hand-written `class="button"` (28 sites in 11 files): there is no plain `button`/link-button
  component (`src/web/ui/button.rs` has `add_link`, `add_post`, `post_button` only), so "where
  components exist" is refuted; it is a class, used as one. `<p class="empty-line">` ×8
  (`instructions/page.rs:81,133`, `agents.rs:220`, `task_wiki.rs:125,148`, `task_runs.rs:287`,
  `task_agent.rs:251,370`) vs `ui/empty_line.rs`: same line count either way; consistency only.
- `task_rows::search` vs `project_rows::project_search_in`: both build a `Search` with their own
  `SYNTAX` and `clear` rule; 10 lines each, different `clear` logic — not worth a generic.
- The `.lost` class: used (`run.rs:171`).

## Proposed `/simplify` pass, in groups that each compile and pass tests

- **Group A — core helpers (S, ~70 lines):** SIM-01 `Db::org_of`; SIM-02 `md_edit::split_front`
  (keep `instruction::split` tests green; check `plan::body_start` line counting); SIM-04;
  SIM-10. Run `cargo test` CLI and web.
- **Group B — process/tmux/db wrappers (S-M, ~75 lines):** SIM-03 (`Runner` defaults first, then
  `Tool` removal, then tmux free fns), SIM-05, SIM-08, SIM-11 (with explicit sort). The
  `FakeRunner` tests in `tmux.rs`/`github.rs` guard this.
- **Group C — CLI/web seam (M, ~25 lines, bug-risk fixes):** SIM-06 `push::prepare` (also halves
  the platform round-trips), SIM-07 `Db::move_task` + key rename. Tests: `push.rs`,
  `web/forms/task.rs`, `web/org_share.rs`.
- **Group D — web plumbing (M, ~90 lines; +180 if discover):** SIM-14 `json()`, SIM-18, SIM-17,
  SIM-15 compose component, SIM-13 asset table; SIM-12 last and separately (dependency change).
- **Group E — CSS (S, ~15 lines):** SIM-19 dead rules and `tasks_tab.css`.
- **Optional:** SIM-20 cfg gating (no lines, smaller CLI binary), SIM-22 `write!`, SIM-23 splits.

## Confidence

High on everything tagged `run` and on SIM-01/02/03/06/07/15 (every site read in full).
Medium on line estimates (±30%), on SIM-13 (the `Route` trait was read but not implemented), and
on SIM-11's ordering remark (inferred from `levels()`; not executed). Low on SIM-12's `discover`
compiling with `default-features = false` (read `topcoat`'s feature list only).

## Out-of-scope observations

- Security: SIM-06's double `push::resolve` means the API key goes over the wire twice per push;
  `platform_pick::open` and `organization.rs:181` build the client the same way — one
  `JsonWebhook::for_org(&org) -> Result<Option<..>>` would also be the single place to add the
  stored-URL re-check the seed asks for (`platform.rs:147` does not re-validate
  `Organization::check`). `process::failure_message` (`process.rs:19`) echoes full argv into
  errors; unchanged by SIM-03 but a natural place to redact.
- Performance: SIM-11 (`list` all projects per wiki load), SIM-06 (two resolves), and
  `RunRecord::shown_status` spawning processes are better covered by the performance lens.
- The CLI-only build has 36 warnings (SIM-20); CI running `--no-default-features` would catch
  future web-only leaks.

## What I could not assess

- `cargo udeps` on the default (web) feature set: nightly 1.91 < topcoat's MSRV 1.98.
- Whether the Oct "`iter` binary 25.7 MB" would shrink measurably from SIM-20 (not built).
- Runtime equivalence of any proposed change — nothing was modified or executed beyond the
  checks above.
