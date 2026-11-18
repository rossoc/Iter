# Self-review of the 2026-11-02 review run (methods, not code)

Written by the orchestrator after reading all ten lens reports in full (3,432 lines), the browser-pass
notes, and re-running six headline claims. The reconciliation report (`review_2026-11-02_summary.md`)
was produced by an eleventh agent in parallel; its spot-check table is referenced, not repeated.

## 1. Corrections to earlier drafts

- The plan's seed list (`doc/review_tools/SEED_FINDINGS.md`) was written from a read-only exploration
  and carried several wrong or overstated items. Refuted with evidence by the lenses: ureq forwards no
  `Authorization` on redirects (default `RedirectAuthHeaders::Never`); `security -i` does report
  failures (exit 44); `Plan::days` is sorted by construction (BTreeMap); `plan.rs:344` slicing is
  ASCII-safe; the `TaskStatus` → `Queue` fallback is unreachable (CHECK constraint); all five indexes
  exist on a fresh store; `migrate()` after the first open is one pragma read, not a cost; `rand 0.8`
  is not in the graph; `clap_complete` `unstable-dynamic` removes 2 crates, not 22; `.lost` CSS is
  used; task validation CLI vs web is not a duplicate; `POST /task/new` does re-render with the error
  (only four pop-up routes answer bare 400, not five).
- My browser-pass note said the 360 px overflow "looks like a regression from the added nav items".
  The accessibility lens pinned it to commit `0a3f175` (2026-10-31) by measurement; the design lens
  measured 654 px on 46 of 49 pages. Both confirm; my hypothesis about `--main-w` was wrong, the
  cause is the un-wrapping `.bar` plus the `nowrap` project path.
- The plan listed "footer opens a second DB connection" as a performance seed. True, but the measured
  cost is 0.6 ms per open on a 1.5 ms page: real but Medium, not the headline the seed implied.

**Reconciliation outcome** (`review_2026-11-02_summary.md`): 133 distinct findings after merging 17
cross-lens duplicates (Blocker 2, High 14, Medium 45, Low 72). Of 30 spot-checks, 28 held, one held with a
wrong line citation (`git.rs:529` is `git.rs:69`), one partly (the saved cargo-check log is the
`--all-targets` variant); none failed. My own six re-runs (guard scheme gap, foreign Origin 403,
case-sensitive Host, `to=9999-12-31` empty report, footer margin, `syn 2`, no `.bar` wrap, instruction
layer mismatch) all held.

## 2. What was under-specified

- **Request**: "access the app remotely" was ambiguous; asked and resolved as "drive it in a browser"
  before the run. Good. "Faster code" was not scoped to CLI vs web; the performance lens covered both,
  which cost it ~19 minutes of wall time.
- **Prompts**: the brief said "write exactly one report" but did not cap report length; the composability
  report is 866 lines, mostly raw SCC edge lists that belong in `out/metrics.md`. Next run: cap the
  report at ~300 lines and point to the tool output.
- **Environment**: the brief said headless Chrome exists but Playwright does not. Two lenses installed
  puppeteer/Playwright into their tool folders anyway (and cleaned `node_modules`). Acceptable, but the
  brief should say whether `npm install` is allowed and where.
- **Severity**: the rubric's "fails WCAG Level A = Blocker" produced a Blocker (`/` shortcut, 2.1.4)
  that the lens itself judged low practical risk. The rubric needs an "impact override" clause.
- **Evidence labels**: `run` was applied consistently; `read` vs `inferred` was blurred in two reports
  (composability's "compiles on its own" is inferred and said so; simplification's line estimates are
  ±30 % and said so). Fine.
- **Overlap**: the brief did not say which lens owns the pop-up bare-400 issue; five lenses reported it
  (correctness_web CW-5, security SEC-13, design DC-04, accessibility A11Y-07, testing indirectly).
  Convergence is useful once, not five times; assign primary ownership next time.

## 3. Good results and how they were achieved

- **Every Blocker/High has `run` evidence**: clickjacking (screenshot inside a cross-origin frame),
  key leak (bearer header seen at a sink host after saving the URL through the real form), process
  group kill (exit 143 demonstrated), double push (two ledger rows against a fake platform),
  instruction-layer mismatch (200 on preview, 400 on submit), clone-into-itself (176 nested dirs),
  1698 ms `/agents` at scale 16 isolated to process spawns by rewriting the records. The brief's rule
  "Blocker/High needs run or very precise read" did this.
- **Seeded stores at three scales plus a filesystem filler** made the performance numbers meaningful
  and showed what is *not* a problem (every DB page 1.2–2.6 ms at ×16, ratios ≤1.7).
- **Refutation was rewarded**: the brief said "confirm, refute or ignore with evidence", and the lenses
  refuted 15+ seed and skill claims. The code-review skill's 18 + 17 candidates were treated as
  hypotheses; one was refuted outright, several downgraded.
- **Independent convergence**: my browser pass, the design lens and the accessibility lens found the
  360 px overflow separately (none read the others). Same for the agent-writable `pid` (security,
  performance, testing, correctness_core all flagged it from different angles).
- **Feasibility proofs**: the testing lens built a standalone topcoat project proving in-process HTTP
  tests and a node/jsdom runner, so "add router tests" is now an S/M task, not a research item.
- **Build lens ran 17 real builds** in a copy with separate target dirs and reported variance (±20 %
  clean) honestly; it ruled out `panic=abort` by reading topcoat's `catch_unwind` sites rather than
  guessing.

## 4. Weak results, why, and how to prevent them

- **Report bloat** (composability 866 lines, accessibility 455, security 402): the skeleton asked for
  tables and details and did not cap. Prevention: a length cap and "raw output goes to
  `review_tools/<lens>/out/`".
- **Chrome extension not connected**: my own browser pass fell back to headless screenshots; no
  console or network inspection happened from my side. The lenses covered console-free checks via
  Playwright. Prevention: check the extension before planning the browser pass, and plan the fallback.
- **No concurrency measurement**: every lens ran one client at a time; `block_in_place` behaviour under
  parallel `/search` keystrokes is still unmeasured (it was also unmeasured in October).
- **Agent runner never exercised**: no lens launched `claude` (by design, to keep the machine clean), so
  SEC-04's chain and `/agent/{id}/{n}` pages are read-confirmed only. Prevention: a fake `claude`
  binary on `PATH` in the scratch env would make the run path testable.
- **`cargo udeps` not run on the web feature set**: nightly 1.91 < topcoat MSRV. Prevention: note the
  toolchain requirement in the brief or allow `rustup update nightly`.
- **Cold build in the worktree** cost 1m50s + 43s test build before anything ran; acceptable once, but
  the project itself creates a worktree per task with no shared `CARGO_TARGET_DIR` (build lens
  observation), so every task pays it.
- **Duplicate seed scripts**: three lenses wrote their own node screenshot/axe harness. A shared
  `review_tools/browser/` helper would have saved time.

## 5. Per-agent method table

| Lens | Good | Missing | Bad |
|---|---|---|---|
| security | Threat model stated; 15-case guard matrix; PoCs saved; refuted 2 seeds; remote-access section answers the user's question | Real `claude` run; Linux `/tmp` | Rubric-literal Blocker for clickjacking (self-flagged as High/Blocker boundary) |
| correctness_core | Scripted editor for CLI repros; 8 refutations with evidence; `repro.sh` reproducible | Network paths (gh, platform) run | Severity of C3 (documented `branch -D`) debatable, said so |
| correctness_web | Fake platform server; sequential lost-update repro; 9 refuted hypotheses listed | Browser-side JS behaviour (read only) | None |
| performance | Three scales + FS filler; cause isolation by removing it; EXPLAIN on 18 statements; "found optimal" list | Concurrency; rusqlite open cost approximated with Python | None |
| simplification | Clone detector with two modes; dead-CSS script fixed after a false positive; grouped fix plan | `cargo udeps` on web set | Line estimates ±30 % |
| composability | Script with SCC/fan-in/out; every cycle edge read-confirmed; 11-step ordered plan | Did not run tests for "compiles on its own" (labelled inferred) | 866-line report; raw edge lists inline |
| design_consistency | 196 screenshots with the site's real dark theme; `inventory.json` of DOM facts; `overflow.mjs` names culprits | Hover/drag states; `/agent/{id}/{n}` | Looked at 20 of 196 screenshots |
| accessibility | 98 axe runs with the animation artefact identified and excluded; real keyboard walks; computed contrast matched to Chrome's values; Oct A1–A7/F1–F14 re-checked one by one | Real screen readers; Safari/Firefox | Rubric-literal Blocker for `/` |
| testing | Ran suite 4 ways; two feasibility proofs that compile and pass; branch map for 11 risky modules | Coverage tool | None |
| build | 17 timed builds, variance reported; `-Z time-passes`; nm-based size attribution without installing tools | Runtime effect of -Os; Linux | Clean-time noise ±20 % limits some comparisons |

## 6. What to change next run; suggested next code steps (not done)

Method changes:
1. Cap reports at ~300 lines; raw output to `review_tools/<lens>/out/`.
2. Assign a primary owner per cross-cutting issue (pop-up 400s, agent-writable pid, DB opens).
3. Provide one shared browser harness (Playwright + axe + screenshot) before the fan-out.
4. Put a fake `claude` on `PATH` in the scratch env so the agent runner can be exercised.
5. Add an "impact override" to the severity rubric for rubric-literal Blockers.
6. Check the Chrome extension connection before planning a browser pass.

Code steps the user should approve (the ranked backlog is in `review_2026-11-02_summary.md`):
- Batch A (security, S): frame/CSP/nosniff/referrer header layer; parse `platform_url` and refuse
  userinfo, re-check on read; POST-only platform-activities; `--` + `allow_hyphen_values` on hook args;
  validate `pid` before `kill`; `env_remove` the key on spawns; prompt via stdin.
- Batch B (correctness, S–M): honour `already_pushed` and 303 after push; task-layer instructions in
  `create_run`/`edit_run`; CRLF normalisation on prompt and session note; clone dest-inside-src guard;
  trim CLI task names; `=name` tmux targets; `to=9999-12-31` clamp.
- Batch C (performance, S): one `ps` per render; cached search entries; `ingest::mode` before graph
  build; one `Db` per request shared with the footer; `usage()` without per-task org query.
- Batch D (web a11y/design, S–M): `.bar` wrap under 600 px; project path shrinkable; footer link
  height; muted-on-tint colour; pop-up refusals re-render with `FormError`; one close idiom; `/agents`
  and `/instructions` filter bar; error page through `frame`; docs refresh.
- Batch E (simplification/structure, S–M): `Db::org_of`, one front-matter splitter, `Runner` cleanup,
  `push::prepare`, asset-route table, JSON helper, compose component, dead CSS; `write_atomic` to
  `utils/fs.rs`; cfg-gate web-only agent code.
- Batch F (build, S): `[profile.dist]`, `[profile.dev]` debug settings, `syn = "3"` in macros.
- Batch G (tests, M): in-process router harness with guard cases; JS runner with exported helpers;
  midnight/DST/migration-resume tests.
