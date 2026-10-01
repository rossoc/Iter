# Self-review of the six-reviewer run (revision 2)

Revision 2: I read all six reports in full (1,168 lines) and re-checked four headline claims myself. The first version was written from summaries and a few greps, and it contained errors. They are listed in section 0.

This document judges the **methods** of the reviews, plus how I specified the task. It does not re-review the code.

## 0. Corrections to my first version

| First version said | What the reports show |
|---|---|
| DRY has an "unquantified clone threshold". | Wrong. The DRY report states it: comments stripped, literals replaced by placeholders, a 4–5 line sliding window, a second variant that also collapses identifiers. The real weakness is different: the scripts live in the session scratchpad and are not in the report, so the run cannot be reproduced. The same holds for the composability scripts. |
| Overlaps were "CSS-coupled sizes, no Esc on the tip, and similar". | Mostly invented. The real overlaps are listed in section 3. |
| Side findings such as the bare 404 and `<title>` in `<body>` "belong to no lens". | Wrong. The bare 404 is design L5 and the `<title>`/`<link>` in `<body>` is accessibility F7. Only the `str_to_dt` panic (complexity) is truly out of lens, and the complexity agent flagged it itself. |
| The design agent could not seed data because of how I scoped it. | Partly wrong. The accessibility and complexity agents seeded a throwaway database directly with `sqlite3` or scripts. The design agent assumed seeding needed the interactive editor. That was avoidable and was the agent's gap, which my prompt did not close. |
| "Same skeleton" makes the reports comparable. | Only at heading level. Inside, evidence labels differ (see section 4). |
| Testing claim unverified. | Now verified: `cargo test --features web --no-run` fails with 3 errors, `with_side` not found at `agenda_pick.rs:101`. The one-line import fix is the compiler's own suggestion. |
| Not verified before. | Also confirmed by me: `add_button` has one hit, its definition (`ui/button.rs:22`); `save_task` is `#[cfg(feature = "web")]` (`db.rs:580`); `<title>` and `<link>` are emitted inside the frame view (`ui/frame.rs:53-57`). |

## 1. What I under-specified in the request and prompts

1. **"Asymptotically optimal" was undefined.** The complexity agent had to decide whether n means code input or page cost. It chose well (it measured and reported bounds, not just exponents), but it was luck.
2. **No shared environment recipe.** The accessibility and complexity agents each built their own seeded database, scratch `XDG_CONFIG_HOME` and headless Chrome. The design and testing agents did not. Design therefore stayed mostly source-inferred and says so. I knew a running app was needed and did not provide a recipe.
3. **No common severity scale.** The reports use High/Medium/Low (accessibility, composability H/M/L, DRY, design), Critical (testing F1), and "Medium (independent of the page…)" with prose in complexity. The same issue is rated differently (hover-only card action: accessibility Low F10, design Medium M8). The reports cannot be merged into one ranked list.
4. **No common evidence labels.** Accessibility labels each finding "verified"/"inferred"; complexity uses `[run]`/`[inferred]`; design uses a confidence table; DRY and composability use one global statement. I asked for "verified vs inferred" but not in what form.
5. **Scope.** `src/web` was clear. I did not decide whether `utils/`, `reporting.rs`, `git.rs`, `github.rs` and the CLI were in or out. DRY looked at `commands/` and found its best finding there (F1, F2: non-atomic CLI task write and duplicated templates). Testing counted tests outside the web, but nobody reviewed them. The request spoke of "the code"; I narrowed it silently.
6. **"Accessibility, accessibility and accessibility"** gave priority but no standard. I picked WCAG 2.2 AA in the prompt; the user did not.
7. **Independence rule had a cost.** I forbade reading other reports. It gave independent evidence, but there was no later reconciliation step.
8. **No requirement to leave scripts behind.** Several reviews depend on throwaway scripts that are not reproducible.
9. **No "out-of-scope observations" slot** and no half-page summary requirement.

## 2. Good results, and how they were achieved

- **Accessibility (best).** Real app, throwaway DB, 17 pages × light/dark with axe-core, Tab walks, 320/400 px reflow, target-size measurement, computed contrast for every token. Findings F2, F3 and F6 were reproduced in Chrome. Each finding says how it was established, and the agent separates "verified by axe" from "inferred" and lists what it could not test (screen readers, Firefox/Safari, forced colors). It also reports what is good, which tells the reader which areas need no work.
- **Complexity.** It measured scaling (1k–64k tasks, 250–16k projects, with status and byte counts to avoid timing 500s) and found no hidden O(n²). The conclusion is "the bounds are wrong, not the exponents": the footer is O(P+O+B) on every page, and the report reads all rows for a few lines of output. It also has a "checked and found optimal" list, so the absence of findings is explained, and a function-by-function table.
- **Composability.** The module graph was built by script and checked with `cargo check`. It yields three concrete cycles, fan-in counts and call-site counts, plus an ordered simplification plan. The "what is good" section keeps the layering finding (`ui/` never imports page code).
- **DRY.** It states clearly that textual clones are few and the real redundancy is the same fact written in several places. That is a useful reframing. F1 (non-atomic CLI write vs. transactional web write) and F3 (error-to-field mapping by message prefix) are real design risks, not style.
- **Testing.** It ran the suite and found the compile failure, which no other lens could have found. It verified the one-line fix in a scratch copy (302 pass in 0.56 s). The missing-test list is concrete (guard edge cases, `drop_on` error paths, `settle`, escaping).
- **Cross-lens convergence** (independent agents finding the same thing, which raises confidence):
  - `add_button` unused: composability M5 and design M1, both with a grep; I re-checked it.
  - Hover-only card action: accessibility F10 and design M8.
  - "Backlog"/"Unscheduled" naming: design L1 and DRY F12.
  - The new-project/new-task modal cluster is a problem structurally (composability H1/H2) and as duplication (DRY F11), and both agree on a `Home`/`TaskHome` merge.
  - The footer: complexity F1 (O(P) on every page), accessibility F3 (focusable behind a modal), design M7 (duplicates navigation). Three lenses converge on one component.
  - Form skeleton repeated: composability M2, DRY F10, design M4/L6.
  - Hard-coded breakpoints and sizes: DRY F7/F8/F9, with accessibility confirming 900/600 px reflow works.

## 3. Bad or weak results: why, how to prevent, how to fix

| Weak result | Why | Prevention | Fix |
|---|---|---|---|
| **Design consistency is mostly inferred.** Populated pages, the modals, dark and narrow layouts and the pick mode were never rendered, yet it rates "overload" (M7) and discoverability (M8). | Agent believed seeding needed the editor, while two sibling agents seeded with `sqlite3`. | Shared setup recipe with a seed script. | Rerun with the seed and screenshots of every page in both themes and at narrow width. |
| **Judgement findings are rated with the same confidence as mechanical ones.** Composability H1–H3 are rated High ("structural"), but cycles compile and have no user or runtime impact; the "god module" calls come from headers and function lists (the report admits it read only a few files fully). | No rubric for severity, no cost/benefit field. | A severity rubric that separates "user impact", "bug risk" and "maintenance cost". | Re-rate with the rubric. |
| **Reports are not reproducible.** Clone and graph scripts, seed generators and puppeteer scripts stayed in the scratchpad. | I asked for methods in prose, not artifacts. | Require scripts under `doc/review_tools/` or inlined. | Save them now (they still exist in this session's scratchpad). |
| **Performance numbers are from a debug build, warm cache, no EXPLAIN.** Growth is meaningful; absolute times and the partial-index claim (F4) are not. | Existing debug binary reused. | Specify `--release` and `EXPLAIN QUERY PLAN`. | Re-measure. |
| **Testing's main proposals depend on an unverified fact**: whether `topcoat` can render to a string or provides a test client, and whether it escapes by construction (M1, M2, M6 are medium-low). | Scope stopped at the crate boundary. | Name the one dependency question to answer. | Read `topcoat`'s API; one test with `<script>` settles M2. |
| **No reconciliation or ranking.** Six reports, ~1,170 lines, 70+ findings, mixed severities, duplicates. A reader cannot tell what to do first. | I did not plan a merge step or a summary requirement. | Common ID/severity/effort schema and a half-page summary. | A seventh pass producing a single ranked backlog. |
| **Unexamined conflicts.** Examples: severity of the hover-only action differs; accessibility treats `<h2>` in `<summary>` (F5) as a defect while composability proposes more folding components; composability proposes merging many small `ui` components while accessibility and DRY rely on those components' consistency. No agent weighed the trade-off. | Independence by design, no arbiter. | Reconciliation step. | Decide per conflict. |
| **Coverage gaps remain.** `utils/`, `reporting.rs`, `git.rs`, `github.rs`, `sync.rs`, the CLI; the two JS files were read for a11y and DRY but never for testing/composability; no lens looked at security (the testing agent found `guard.rs` untested, and `folders.rs` lists any directory). | Scope and lens list were mine. | Decide scope explicitly and add a security lens if wanted. | Add passes. |
| **Headline claims rest on one agent.** I re-checked four (compile error, dead `add_button`, `save_task` gate, `<title>` in view) and all held, but the accessibility Chrome results, the scaling numbers and the cycle counts were not re-checked. | No verification step. | Second-agent spot checks of each report's top three findings. | Do that now. |
| **Wasted overlap** of effort: three agents each rebuilt a seeded environment; two each built script harnesses. | No shared tooling. | Shared tooling. | Same recipe. |

## 4. Method evaluation per agent

| Agent | What was good | What was missing | What was bad |
|---|---|---|---|
| Accessibility | Real browser plus axe plus manual probes; per-finding evidence level; strengths listed; approximation flagged (F12) | Screen readers, Firefox/Safari, text spacing, touch drag, forced colors rendering, un-seeded pages | Nothing serious. Some findings (F10, F11, F13) are reasoned only, and the report says so |
| Complexity | Measured scaling with validity checks; "found optimal" list; table | Release build, EXPLAIN, concurrency, browser cost of 20–66 MB pages | Debug-build constants; some claims (index, double sort) inferred |
| Composability | Scripted SCC and fan-in, `cargo check`, call-site counts, ordered plan | Item-level graph, thresholds for "god", `board.js`/`org.js`, test impact of proposed moves | Subjective judgements rated as High; Mermaid graph is hand-drawn from the script output, not generated |
| DRY | Stated clone method, semantic grep passes, cross-checks against `commands/`, honest that clones are few | `utils/`, `git.rs`, `reporting.rs`; never compiled or ran anything | Scripts not preserved; some findings (F14–F16) are cosmetic and dilute the list |
| Design consistency | Per-page inventory table; compared with `design/04`; ran the app with an empty DB | Populated pages, screenshots, dark/narrow | Mostly source-inferred; judgements of density (M7) are unsupported by renderings |
| Testing | Ran the suite; found a blocker; scratch-copy fix; concrete test proposals | Coverage, flakiness, JS, `topcoat` test API | Some "missing test" priority is guessed without knowing the library's guarantees |

## 5. What I would change next time

1. One shared setup doc: seed script, scratch `XDG_CONFIG_HOME`, `--release` build, headless Chrome, where to put scripts.
2. A finding schema: ID, lens, severity (rubric), `file:line`, evidence (`run`/`read`/`inferred`), effort, fix.
3. A required half-page summary and an "out-of-scope observations" section per report.
4. A reconciliation pass after the parallel run: dedupe, resolve conflicts, rank, re-check top claims.
5. Explicit scope, including non-web code, and an explicit decision on a security lens.
6. Ask for scripts to be saved, so the review is reproducible.

## 6. Suggested next steps for the code (from the reports; not done)

1. Fix the test compile error (verified): add `use crate::web::url::with_side;` to the `agenda_pick.rs` test module, then run the suite.
2. Accessibility F1–F3 (cheap and reproduced): `code` colour, agenda date auto-submit, modal focus containment.
3. Footer: cap or make it static, and reuse the page's connection (complexity F1; it also helps accessibility F3 and design M7).
4. Ungate `save_task` and use it from the CLI (DRY F1).
5. Break the three cycles (composability), starting with `ui/field_ids.rs` and moving `NAME`/`DESCRIPTION`.
6. Rerun the design review with rendered pages.
