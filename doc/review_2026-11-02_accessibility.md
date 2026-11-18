# Accessibility review (WCAG 2.2 AA) — 2026-11-02

Branch `feat/review`, HEAD `5d96f44`. Lens: accessibility only. No source edited. Everything below was
checked against running pages (the shared scale-4 server on :3999 for GETs, my own scale-1 store on
:4020 for anything that posts) in Google Chrome 156 driven by Playwright 1.52, with axe-core 4.10.3.

## Summary

The web UI is in good shape for a server-rendered app: every control is labelled, every page has one
`h1`, labelled landmarks, a skip link, `aria-current` on tabs, `role=status`/`role=alert` where things
change, a complete keyboard path for every drag-and-drop action (pick mode), working focus trap and
Esc on the pop-ups, reduced-motion and forced-colors fallbacks that all hold up under emulation, and
refused forms that re-render with a focused, linked error box. Of the Oct 2026 fixes A1–A7, six hold and
one (A6) is insufficient; of the Oct findings F1–F14, eight are fixed.

What is not right, top five:

1. **Horizontal scroll on every page at 320 and 400 px** (1.4.10, High, regression since `0a3f175`,
   2026-10-31): the top bar does not wrap, so with four nav items plus the Search button the document is
   654 px wide; "Instructions", "Settings" and "Search" are off-screen. The projects table makes the
   org Projects tab and board Info 1119 px wide at ≤600 px.
2. **Footer project links fail 2.5.8 target size** (High): 17 px tall, 3 px under the organization
   link; axe flags 8 nodes on 74 of 98 page runs (592 of the 717 violation nodes).
3. **`--muted` on `--accent-tint` is 4.32:1 light / 4.14:1 dark** (1.4.3, High): the selected search
   result's subtitle, the combobox's active-item hint and `.drop.over .empty-line` (the Oct A6 fix
   moved the colour to `--muted`, which still fails on the tint).
4. **Refused New note / New session / New instruction / instruction save come back as a bare
   `text/plain` "bad request: …" document** (High): no title, no heading, no landmark, no way back,
   lowercase fragment; the `required` attribute is absent so Enter in an empty field gets there. The
   New task / New project / edit forms show how it should look (focused error box, `aria-invalid`).
5. **`/` opens the search palette anywhere without a way to turn it off or remap it** (2.1.4, Level A,
   so Blocker by the brief's rubric; practical risk is small since it ignores editable fields).

Also notable: the invisible "Go to this date" submit button is a Tab stop with no visible focus; the
card that receives focus after every board action has `outline:none`; search results are links nested
in `role=option` (axe `nested-interactive` ×50); the report's disabled period arrows are `span`s with a
prohibited `aria-label`; closing a pop-up drops focus to the top of the document.

## Scope

- Routes (47 GET pages + 2 script states, light and dark): `/`, `/settings`, `/org/1` with every tab
  and the `?new=project`, `?new=task`, `?share=1` pop-ups, report with a bad date, `/org/1/edit`,
  daily notes (`/org/1/notes/2026-11-02`, `?tab=edit`, `/project/1/notes/2026-11-02`), `/project/1`
  (info, tasks, tasks+new, notes, edit), `/project/5` (no org), `/task/1` with
  `?tab=info|sessions|notes|agent|wiki` and the session/note/agent pop-ups, `/task/1/edit`, `/agents`,
  `/boards`, `/board/2` agenda (`?date=`, `&side=off`, `&pick=2`, `&moved=2`), `/board/2/matrix`
  (`?pick=24`, `?moved=24`), `/board/1/info`, `/board/1/edit`, `/instructions`,
  `/instructions?new=instruction`, `/instruction/develop` (+ `?tab=settings`), `/org/x` (400),
  `/org/99999` (404), `/nope` (404). Board 2 was used on the shared store because every task scheduled
  today on board 1 is `done` and the board shows unfinished cards only (pick mode never opened).
- POST paths on my own store: `/task/new`, `/task/1/notes`, `/task/1/sessions`, `/instructions`,
  `/instruction/develop`, `/org/1/project/quick`, `/task/1/edit`, `/board/1/schedule`,
  `/board/1/matrix` (pick-mode buttons).
- Source read: `src/web/base.css`, `src/web/org.css`, every `src/web/ui/*.css`, `ui/{frame,modal,
  search,combobox,filter_bar,field,form,form_error,notice,table,task_card,tag_chips,ink,tabs,
  breadcrumb,day_nav,hour_grid,pick_bar,pick_here,drop_zone,unplaced_list,folded_list,side_rail,
  sessions_table,task_table,quadrant,status,count,panel,notes,group,button,layout,page_header,
  site_footer,empty_state,labelled,project_list}.rs`, `ui/{modal,search,combobox,filter_bar}.js`,
  `board.js`, `org.js`, `layout.rs`, `errors.rs`, `task_new.rs`, `task_notes.rs`, `task_sessions.rs`,
  `instructions/{mod,list,save}.rs`, `org.rs`, `org_report.rs`, `org_share.rs`, `daily_notes.rs`,
  `agents.rs`, `task.rs`, `sections.rs`, `matrix.rs`, `quadrants.rs`, `agenda*.rs`, `board_*.rs`,
  `pick.rs`, `crumbs.rs`.

## Method

Scripts live in `doc/review_tools/accessibility/` (run with `node <script>` from that folder after
`npm install playwright@1.52.0 axe-core@4.10.3`; Playwright's own Chromium download was incomplete on
this machine, so `lib.js` launches the installed Google Chrome via `channel: "chrome"`). Outputs are
under `doc/review_tools/accessibility/out/` (axe JSON per page and scheme in `out/axe/`,
`axe_summary.md`, `axe_recheck.log`, `keyboard.log`, `forms.log`, `media.log`, `live_contrast.log`,
`extra.log`, `reflow.json`, `targets.json`, `audit.json`, `obscured.log`, `apply.log`, screenshots in
`out/shots/`).

- `axe_run.js`: axe-core (tags wcag2a/aa, wcag21a/aa, wcag22aa, best-practice) on 49 states × 2
  colour schemes at 1280×900. `axe_recheck.js`: the six pop-up pages again 700 ms after load and
  under reduced motion (see "artefact" below).
- `keyboard.js`: real Tab/Shift+Tab/Enter/Escape/Arrow presses: Home order; agenda card action →
  pick → hour → moved → Cancel (own store, task 6); matrix Place (task 1); New task pop-up (trap, Esc,
  ✕, backdrop, focus return); search palette (`/`, button, arrows, Tab into the preview, Esc, Enter);
  both combobox kinds; filter bar and its tip; report date form; drag-and-drop alternative counts.
- `forms.js`: each pop-up form submitted empty on :4020; records title, content type, landmarks,
  `role=alert`, focus, `aria-invalid`, kept input, and what Back restores.
- `reflow.js`: every route at 320 and 400 px: `scrollWidth`, elements past the viewport, clipped text.
  `extra.js`: the overflow culprits, footer link spacing, instruction headings, compose dialog, pick
  banner edge, accessibility-tree snapshots, 320/400 px screenshots.
- `targets.js`: every link/button/input measured at 1280 and 400 px with the 24 px spacing exception.
- `audit.js`: emitted DOM per page: title, `h1` count, heading order, landmark labels, table captions,
  `aria-current` values, duplicate ids, dangling `aria-*`/`for`/`list` references, alerts, inert parts
  under a dialog, unlabelled controls.
- `media.js`: `prefers-reduced-motion: reduce` and `forced-colors: active` emulated; computed styles
  of nav, tabs, presets, cards, picked card, pick slot, drop zones, modal, focus rings, breathing dot.
- `contrast.js`: WCAG ratios from the `:root` tokens of `base.css` (light and dark), reproducing
  `color-mix(in oklab)` for `--accent-tint` and the sRGB mixes of the component sheets; chip ink per
  the `ui/ink.rs` rule. `live_contrast.js`: the same pairs read back from Chrome's computed colours
  (canvas-resolved), including state-dependent ones (selected result, active combobox item, `.over`).
- `obscured.js`: 2.4.11, Tab through agenda/pick/report/sessions at 1280×800 and 1000×620, checking
  the focused element against the sticky bar, sticky head, aside, panel and TOC.
- `curl -i` for the raw refusals and the 400/404 documents; `sqlite3` for ids.

Not checked: see the last section.

### axe artefact worth knowing

On the six pop-up pages axe (sampled right after `load`) reported 1–6 `color-contrast` nodes in light
mode with blended colours (`#817e78` on `#fbfaf8`, `#f2f4fc` on `#5470d8`). Those are the 250 ms
`modal-in` entrance animation (`ui/modal.css:14-19`): the text was sampled mid-fade. Re-run 700 ms
later, or with reduced motion, all six pages have 0 violations (`out/axe_recheck.log`). The 17 nodes
are excluded from the counts below.

## axe violations per page (nodes; light / dark)

| Page | URL | Light | Dark | Rules |
|---|---|---|---|---|
| home | `/` | 8 | 8 | target-size |
| settings | `/settings` | 8 | 8 | target-size |
| org-info | `/org/1` | 8 | 8 | target-size |
| org-projects | `/org/1?tab=projects` | 8 | 8 | target-size |
| org-projects-new | `/org/1?tab=projects&new=project` | 0 | 0 | — |
| org-tasks | `/org/1?tab=tasks` | 8 | 8 | target-size |
| org-tasks-new | `/org/1?tab=tasks&new=task` | 0 (5 artefact) | 0 | — |
| org-notes | `/org/1?tab=notes` | 8 | 8 | target-size |
| org-report | `/org/1?tab=report&from=…&to=…` | 9 | 9 | aria-prohibited-attr, target-size |
| org-report-bad | `…&from=nonsense…` | 9 | 9 | aria-prohibited-attr, target-size |
| org-share | `…&share=1` | 0 (1 artefact) | 0 | — |
| org-edit | `/org/1/edit` | 8 | 8 | target-size |
| org-note | `/org/1/notes/2026-11-02` | 8 | 8 | target-size |
| org-note-edit | `…?tab=edit` | 8 | 8 | target-size |
| project-info | `/project/1` | 8 | 8 | target-size |
| project-loose | `/project/5` | 8 | 8 | target-size |
| project-tasks | `/project/1?tab=tasks` | 8 | 8 | target-size |
| project-tasks-new | `…&new=task` | 0 (6 artefact) | 0 | — |
| project-notes | `/project/1?tab=notes` | 8 | 8 | target-size |
| project-edit | `/project/1/edit` | 8 | 8 | target-size |
| project-note | `/project/1/notes/2026-11-02` | 8 | 8 | target-size |
| task-info | `/task/1` | 8 | 8 | target-size |
| task-sessions | `/task/1?tab=sessions` | 8 | 8 | target-size |
| task-sessions-new | `…&new=session` | 0 (2 artefact) | 0 | — |
| task-notes | `/task/1?tab=notes` | 8 | 8 | target-size |
| task-notes-new | `…&new=note` | 0 (2 artefact) | 0 | — |
| task-agent | `/task/1?tab=agent` | 8 | 8 | target-size |
| task-agent-new | `…&new=agent` | 0 (1 artefact) | 0 | — |
| task-wiki | `/task/1?tab=wiki` | 8 | 8 | target-size |
| task-edit | `/task/1/edit` | 8 | 8 | target-size |
| agents | `/agents` | 8 | 8 | target-size |
| boards | `/boards` | 8 | 8 | target-size |
| board-agenda | `/board/2?date=2026-11-02` | 8 | 8 | target-size |
| board-agenda-folded | `…&side=off` | 8 | 8 | target-size |
| board-agenda-pick | `…&pick=2` | 8 | 8 | target-size |
| board-agenda-moved | `…&moved=2` | 8 | 8 | target-size |
| board-matrix | `/board/2/matrix` | 8 | 8 | target-size |
| board-matrix-pick | `…?pick=24` | 8 | 8 | target-size |
| board-matrix-moved | `…?moved=24` | 8 | 8 | target-size |
| board-info | `/board/1/info` | 8 | 8 | target-size |
| board-edit | `/board/1/edit` | 8 | 8 | target-size |
| instructions | `/instructions` | 8 | 8 | target-size |
| instructions-new | `…?new=instruction` | 0 | 0 | — |
| instruction | `/instruction/develop` | 9 | 9 | heading-order, target-size |
| error-400 | `/org/x` | 0 | 0 | — |
| error-404 | `/org/99999` | 0 | 0 | — |
| error-404-unmatched | `/nope` | 0 | 0 | — |
| home + palette open | `/` + `/`, "task" | 51 | 51 | nested-interactive ×50, color-contrast ×1 |
| org-tasks + filter tip | `/org/1?tab=tasks` | 8 | 8 | target-size |

Total: 700 violation nodes (350 light, 350 dark; 717 before removing the 17 animation artefacts) from
**five distinct rules**: `target-size` (592 nodes, one cause: footer project links), `nested-interactive`
(100, one cause: palette options), `aria-prohibited-attr` (4, one cause), `heading-order` (2, one
cause), `color-contrast` (2, one cause). Pop-up pages show 0 because axe skips the `inert` page behind.

## Contrast table

Computed from `base.css` tokens (`contrast.js`); `--accent-tint` = `color-mix(in oklab, accent 12%,
bg)` = `#e2e8f6` light / `#3b3642` dark, matching Chrome's computed values to the digit
(`live_contrast.log`). Bold = under the requirement for that use.

| Pair (use) | Need | Light | Dark |
|---|---|---|---|
| `--fg` on `--bg` / `--bg-2` / `--bg-3` | 4.5 | 16.32 / 15.23 / 13.93 | 6.90 / 7.37 / 6.36 |
| `--muted` on `--bg` (labels, hints, nav, th, hour labels) | 4.5 | 5.08 | 5.15 |
| `--muted` on `--bg-2` (kbd, card meta, footer, hints in cards) | 4.5 | 4.74 | 5.50 |
| `--muted` on `--bg-3` | 4.5 | **4.33** (no text instance found) | 4.74 |
| `--muted` on `--accent-tint` (`.palette-sub` selected, `.cb-active .cb-hint`, `.drop.over .empty-line`) | 4.5 | **4.32** | **4.14** |
| `--faint` on `--bg` / `--bg-2` (decoration only: arrows, separators) | n/a | 4.64 / 4.33 | 4.60 / 4.91 |
| `--accent` on `--bg` / `--bg-2` (links, hover, focus ring) | 4.5 / 3 | 5.56 / 5.18 | 7.16 / 7.65 |
| `--accent` on `--accent-tint` (nav current, WIP pill, Today, pick slot hover, pickbar) | 4.5 | 4.72 | 5.76 |
| `--on-accent` on `--accent` (primary button, skip link, brand mark) | 4.5 | 5.80 | 7.16 |
| `--ok` on `--bg` (done pill text) | 4.5 | 5.33 | 7.97 |
| `--danger` on `--bg` / on error-notice bg | 4.5 / 3 | 6.30 / 5.35 | 5.51 / 4.74 |
| `--fg` on error-notice bg (`danger 10% + bg`) | 4.5 | 13.82 | 5.93 |
| `--control-line` on `--bg` / `--bg-2` (input and button borders, link rest line) | 3 | 3.80 / 3.55 | 3.49 / 3.73 |
| `--line-2` on `--bg` / `--bg-2` (status pill, kbd, search button, chip, modal borders) | 3 if a boundary | **1.50 / 1.40** | **1.71 / 1.82** |
| `.status.wip` border (accent 45 %) on tint; `.status.done` border (ok 45 %) on bg | 3 if a boundary | **1.99 / 1.97** | **2.57 / 2.78** |
| `.tcard[aria-current]` dashed accent border; `.post-form.slot` dashed border | 3 | 5.56 | 7.16 |
| Urgent default `#facc15` edge on `--bg-2` card (4 px, words also shown) | 3 | **1.37** | 9.5 |
| Important default `#3b82f6` edge on `--bg-2` | 3 | 3.29 | 4.23 |
| Tag chips with `ui/ink.rs` ink: bug `#ef4444`, docs `#10b981`, research `#8b5cf6`, Urgent `#facc15`, Important `#3b82f6` (all black ink) | 4.5 | 5.58, 8.28, 4.96, 13.71, 5.71 | same |

## Findings

Severity per the brief: Blocker = fails WCAG Level A; High = fails AA; Medium = narrower failure or
clear degradation; Low = tidy-up. Evidence: `run` = executed and observed; `read` = from the source.

| ID | Sev | Criterion | Where | Evidence | Effort | One line |
|---|---|---|---|---|---|---|
| A11Y-01 | Blocker (by rubric; see note) | 2.1.4 (A) | `ui/search.js:227-230`, `ui/search.rs:15` | run | S | `/` opens the palette page-wide; no off switch, remap or focus condition |
| A11Y-02 | High | 1.4.10 | `ui/frame.css:1,6-8`, `ui/frame.rs:11-16` | run | S | Top bar never wraps: 654 px wide at 320/400 px, Settings and Search off-screen on every page |
| A11Y-03 | High | 1.4.10 | `ui/project_list.css:3-8,16-19`, `ui/table.css:31-36` | run | M | Projects table 1099 px wide under 600 px (org Projects tab, board Info) |
| A11Y-04 | High | 2.5.8 | `ui/site_footer.css:14-18`, `ui/site_footer.rs:92-99` | run | S | Footer project links 17 px tall, 3 px under the org link: axe target-size on 74 of 98 runs |
| A11Y-05 | High | 1.4.3 | `ui/search.css:23,28`; `ui/combobox.css:6,9`; `ui/drop_zone.css:8` + `ui/empty_line.css:1` | run | S | `--muted` on `--accent-tint` = 4.32 / 4.14 (selected result subtitle, active combobox hint, `.drop.over .empty-line`) |
| A11Y-06 | High | 2.4.7 | `ui/day_nav.rs:49`, `base.css:55` | run | S | "Go to this date" `button.sr` is a Tab stop with no visible focus (16×6, clip-path) |
| A11Y-07 | High | 3.3.1 / 3.3.3 / 2.4.2 | `task_notes.rs:94`, `task_sessions.rs:81`, `instructions/save.rs:66-79,89-90,143,163,167`, `errors.rs:27-29` | run | M | Refused note/session/instruction forms answer `text/plain` "bad request: …": no title, heading, landmark, link, or kept form |
| A11Y-08 | Medium | 4.1.2 | `ui/search.js:108-127`, `ui/search.rs:37` | run | S | `li[role=option]` wraps an `<a>`: nested interactive ×50; Chrome exposes links, not options |
| A11Y-09 | Medium | 4.1.2 | `org_report.rs:177,182` | run | S | Disabled period arrows are `span`s with `aria-label` (prohibited on generic): name lost |
| A11Y-10 | Medium | 2.4.3 | `ui/modal.rs:23,27`, `ui/modal.js:52-55`, `task_new.rs:109-112` | run | S | Closing a pop-up (✕, Cancel, Esc, backdrop) lands focus on `body`, not on the "New …" opener |
| A11Y-11 | Medium | 1.3.1 (A1 pattern) | `daily_notes.rs:220,343` | run | S | Daily-note pages carry two `aria-current="page"` (crumb and Info/Edit tab) |
| A11Y-12 | Medium | 2.4.7 | `ui/task_card.css:24` | run | S | The card focused after Cancel or a move has `outline:none` though `:focus-visible` matches (F11 re-confirmed) |
| A11Y-13 | Low | 1.4.1 / 1.4.11 | `ui/pick_bar.css:3` vs `ui/notice.css:3`, `ui/mod.rs:81,96` | run | S | Pick banner's 4 px edge is overridden by `.notice{border:1px…}` (cascade order): computed 1 px |
| A11Y-14 | Low | 1.3.1 | `instructions/page.rs` (prose of the prompt), `ui/prose.css:7-9` | run | S | `/instruction/develop`: h2 "Prompt" then h4s from the markdown (`####`) |
| A11Y-15 | Low | 2.4.3 | `task_runs.rs` (`compose_modal`), `ui/field.rs:110` | run | S | New agent pop-up opens without moving focus in (no `autofocus`), unlike the other pop-ups |
| A11Y-16 | Low | 3.2.2 / 1.3.1 | `ui/day_nav.css:12`, `ui/day_nav.rs:42` | run | M | Date being typed is in a 1×1 transparent input: three invisible Tab stops (segments), value unseen (F2 remainder) |
| A11Y-17 | Low | 1.4.13-adjacent | `ui/task_card.css:19-22` | read | S | Card action link invisible until hover/focus on desktop (F10 re-confirmed) |
| A11Y-18 | Low | 4.1.1-adjacent | `ui/frame.rs:46-49`, `layout.rs:24-30` | run | M | `<title>` and three `<script>`s emitted in `<body>` (F7 re-confirmed; browsers cope) |
| A11Y-19 | Low | 1.3.1 | `ui/unplaced_list.rs:30`, `ui/folded_list.rs:21` | run | S | `h2` inside `<summary>` is exposed as "DisclosureTriangle", not a heading (A7 accepted, re-confirmed) |
| A11Y-20 | Low | 1.4.11 | `ui/status.css:1-4`, `ui/search.css:2`, `ui/tag_chips.css:5` | run | S | Pill/search-button/chip hairlines 1.4–2.8:1: decorative since text identifies them, but the only visual boundary |
| A11Y-21 | Low | 2.4.2 / consistency | `errors.rs:14-17` | run | S | 400/404 documents: no "Error:" title prefix, no nav, footer or search; `/nope` is fine otherwise |

### Details

**A11Y-01 · `/` is a character-key shortcut with no off switch.** `search.js:227` opens the palette on
`/` whenever the target is not editable; `aria-keyshortcuts="/"` advertises it. 2.1.4 requires one of:
turn off, remap, or active only on focus. None exists (grep of `src/web` for a preference: none).
Repro: `/org/1`, click the heading, press `/`: palette opens (`keyboard.log` E). It correctly does not
fire inside `#filter-q`. Practical risk is low (speech users rarely dictate "slash"); if the team keeps
the GitHub-style convention, downgrade to High and add a Settings toggle (a `localStorage` flag read by
`search.js`) or require the Search button to be focused. The brief's rubric makes any Level A failure a
Blocker, so it is listed as one.

**A11Y-02 · Reflow: the bar.** `.bar{display:flex;…}` and `.bar nav{display:flex}` have no wrap and the
four nav links are 411 px wide; at 320 px `scrollWidth` is 654 on 44 of 47 pages (`reflow.json`), the
"Settings" link sits at x 413–513 and the Search button ends at 654 (`extra.log`, screenshots
`out/shots/320-_.png`, `400-home.png`). The Oct review recorded no overflow at 320/400; commit
`0a3f175` (2026-10-31) added the Agents and Instructions items. Fix: `@media (max-width:600px){.bar
{flex-wrap:wrap;height:auto;padding-block:6px}}` (and `--bar-h` must then follow, since
`scroll-padding-top` and the sticky offsets read it), or hide the labels under 600 px and give the links
`aria-label`s (keep them ≥24 px).

**A11Y-03 · Reflow: projects table.** Under 600 px `table.css:33-35` turns rows into flex boxes while
the element stays a `<table>`; with `.projects td:first-child{white-space:nowrap}` and the mono `.path`
(1018 px wide, `scrollWidth` 1084) the table grows to 1099 px, so `/org/1?tab=projects`,
`/org/1?tab=projects&new=project` and `/board/1/info` scroll to 1119 px at both 320 and 400 px
(`extra.log` "400px: same"). Fix: `table-layout:fixed;width:100%` or `display:block` on the table under
600 px, and `min-width:0;max-width:100%` on `.path` in that query.

**A11Y-04 · Footer targets.** `.foot-sub li{margin:0 0 3px}` and a 14 px/21.7 px line make each
project link 17 px tall with a 3 px gap to the 20 px organization link above (`extra.log`); the 24 px
circle rule needs ≥3.5 px, so axe reports `closestOffset 23.6 < 24` for the first project of each of
the 8 organizations on every page. Fix: `.foot-sub a{display:inline-block;padding-block:4px}` or
`.foot-sub li{min-height:24px}`; the Oct F9 noted the 3 px margin as thin.

**A11Y-05 · `--muted` on the tint.** Chrome's computed colours: `.palette-sub` of the selected result
`#6f6b64` on `#e2e8f6` = 4.32 (dark `#9699a8` on `#3b3642` = 4.14), the same for `.cb-pop li.cb-active
.cb-hint` and for `.drop.over .empty-line` (`live_contrast.log`); axe flags the palette one in both
schemes. The Oct A6 fix set `.empty-line` to `--muted`, which passes on `--bg` but not on the tint, so
A6 is **insufficient**. Fix: `.palette-list li[aria-selected] .palette-sub, .cb-pop li.cb-active
.cb-hint, .drop.over .empty-line{color:var(--fg)}`; or darken light `--muted` to about `#67635c` and
dark to about `#9fa2b2`, which also lifts `--muted` on `--bg-3` (4.33) over 4.5.

**A11Y-06 · Invisible submit button in the Tab order.** `day_nav.rs:49` renders `<button class="sr"
type="submit">Go to this date</button>`; `.sr` clips it to 1 px and `:focus-visible` cannot show through
`clip-path`. Walk: focus `#day-input`, Tab, Tab, Tab (the three date segments), Tab → `button.sr`
16×6, `clip-path: inset(50%)`, focus ring not visible (`apply.log`, `keyboard.log` B). The report's
Apply button does this right (`org.css:18-19`: static and 58×36 when focused). Fix: `.day-pick
button.sr:focus{position:static;width:auto;height:auto;clip-path:none}` plus the `.button` look, or
drop the button when script is present.

**A11Y-07 · Bare-text refusals.** On :4020, Enter in an empty New note name (`/task/1?tab=notes&new=
note`) navigates to `POST /task/1/notes` → `400`, `content-type: text/plain`, body
`bad request: a note needs a name`, `document.title` empty, no `h1`, no `main`, no link
(`forms.log` 2; same for sessions 3/3b `'' is not a date and time`, `a session must end after it
starts`; instructions 4/5 `'' is not a name: …`; instruction save 6 `an instruction needs a prompt
under its front matter`). A screen reader announces the URL and one lowercase sentence; the only way
on is browser Back (Chrome restored the typed note text; Safari may not). The inputs carry
`aria-required` but not `required` (`field.rs:114`), so the browser does not stop the empty submit. The
New task, New project and task edit refusals do it right: `Error: …` title, focused `#form-error` with
the "Error:" prefix, `role=alert` dropped before focus, link to the field, `aria-invalid`,
`aria-describedby`, "Fix this" cue, typed description kept (`forms.log` 1, 7, 8). Fix: route these four
through the same pattern as `task_new.rs:188-231` (re-render the page with the modal and a
`FormError` built with `sentence()`), and add `required` to the fields so the server path is the
fallback. The brief's seed finding on this is confirmed.

**A11Y-08 · Options that are links.** `search.js:108-127` builds `<li role="option"><a href>…</a></li>`.
axe `nested-interactive` ×50 (light and dark); Chrome's tree lists the listbox children as `link`, so
`aria-activedescendant` points at nodes whose option role is gone (`extra.log` snapshot). Fix: make
the `li` the option (text only, `data-href`), navigate on Enter/click; or drop the listbox/combobox
roles and make it a list of links with arrow-key roving focus.

**A11Y-09 · `aria-label` on a `span`.** `org_report.rs:177,182` render the disabled period arrows as
`<span class="button" aria-disabled="true" aria-label="Previous period">`; `aria-label` is prohibited
on the generic role, so the name is not computed and axe flags it on every report page. Fix:
`<a role="link" aria-disabled="true" …>` or a `<button disabled>` (and `aria-disabled` is then the
visual-only `opacity:.4` rule in `org.css:4`).

**A11Y-10 · Focus after closing a pop-up.** Every close is a navigation to the list URL without a
fragment, so the document starts at the top (`keyboard.log` D: Escape → `active=BODY`); the "New
task…" row is on the page but not focused. Pick mode shows the fix: its Cancel goes to `#card-N` and the
card takes focus (`keyboard.log` B). Give the add row an id (`tr.add` or the `.add-link`) and append
`#new-task` to the `close` URLs in `task_new.rs:109-112`, `task_notes.rs:32`, `task_sessions.rs:31`,
`instructions/save.rs:23`, `org.rs:122-126`.

**A11Y-11 · Two `aria-current="page"`.** `/org/1/notes/2026-11-02` and the project note have the
crumb `<span aria-current="page">2 November 2026</span>` (`Crumb::here`, `daily_notes.rs:220`) and the
current tab (`note_tabs`), the exact situation A1 fixed elsewhere by using `Crumb::label` when the page
has tabs (`audit.json`: `aria-current=page x2` on `org-note`, `org-note-edit`, `project-note`).

**A11Y-12 · Focus lands on an unmarked card.** After "Move here · 00:00" the server redirects to
`#card-6`; Chrome focuses the card (`activeElement = article#card-6`, `:focus-visible` true) but
`task_card.css:24` sets `outline:none` (`keyboard.log` B: `no-ring`). The same after Cancel. The
status region does announce "Moved project-1/task-6 to Mon 2 Nov, 00:00" (`role=status`, text
rewritten 100 ms after load as `board.js:62-70` intends). Fix: replace `.tcard:focus{outline:none}` by
`.tcard:focus:not(:focus-visible){outline:none}`.

**A11Y-13 · Pick banner edge.** `pick_bar.css:3` says `border-left-width:4px` and the comment calls it
"a shape as well as a color"; `notice.css:3` (`border:1px solid`) is concatenated later (`ui/mod.rs:81`
vs `:96`) and wins: computed `border-left-width: 1px` in pick mode (`extra.log`). The text in the banner
carries the meaning, so Low; fix by moving `pick_bar.css` after `notice.css` or raising specificity.

**A11Y-14 · Heading skip.** `/instruction/develop`: `H1 Develop, H2 Prompt, H4 The task, H4 …, H2
Details` (`audit.json`); the markdown of the prompt uses `####`. Either demote via the prose renderer
(shift levels so the first prose heading is one below its section) or document that instruction bodies
start at `###`.

**A11Y-15 · New agent dialog focus.** `/task/1?tab=agent&new=agent` opens the "New agent" dialog with
`activeElement = body` and no `[autofocus]` (`extra.log`); the other pop-ups autofocus their first
field (`field.rs:110`). The inert page means the first Tab does land inside, so Low.

**A11Y-16 · Hidden date input.** Three Tabs through an invisible 1×1 control (`apply.log`: segments
month/day/year) before the hidden button; the value being edited is never visible (`day_nav.css:12`).
F2's navigation-per-keystroke is fixed (ArrowUp no longer navigates; Enter applies), the visibility
part remains. Consider a visible date field under 900 px or beside the title.

**A11Y-20 · Hairline boundaries.** `.status` pills and `.search-btn` use `--line-2` (1.5:1 light),
`.status.wip/.done` use 45 % mixes (2.0–2.8:1). 1.4.11 exempts a boundary that is not needed to
identify the component (the text does), hence Low; `--control-line` would make them 3.5:1.

### Oct 2026 fixes A1–A7 and findings F1–F14: re-check

| Item | Status | Evidence |
|---|---|---|
| A1 two `aria-current="page"` | holds on report (`page` tab + `true` preset) and boards (`true` nav + `page` tab); **regressed on the daily-note pages** (A11Y-11) | `audit.json`, `keyboard.log` H |
| A2 forced-colors cue on nav and presets | holds: nav current underline + 2 px Highlight outline, presets 2 px outline, tabs underline + border | `media.log` |
| A3 report error `role=alert` | holds: "Dates are YYYY-MM-DD; showing the last 7 days." | `keyboard.log` H |
| A3b Apply reachable | holds: 4th Tab from "To" (after its segments) reaches `button.apply`, 58×36, visible | `apply.log` |
| A4 error link to a fieldset | holds (`group.rs:21` gives the fieldset `tabindex=-1`) | read |
| A5 secondary button border | holds: `--control-line` 3.80 / 3.49 | `live_contrast.log` |
| A6 `.drop.over .empty-line` | **insufficient**: `--muted` on the tint is 4.32 / 4.14 (A11Y-05) | `live_contrast.log` |
| A6b error pages through a frame-like document | holds: 400/404 and `/nope` have title, skip link, one `main`, one `h1`, home link; POST stays raw (A11Y-07) | curl, `audit.json` |
| A7 heading in `<summary>` | unchanged, accepted (A11Y-19) | `extra.log` snapshot |
| F1 `code` in muted text | fixed (`base.css:57` sets the colour; no axe hit) | run |
| F2 date field navigates per keystroke | fixed (ArrowUp does not navigate, Enter does); hidden-input part remains (A11Y-16) | `keyboard.log` B |
| F3 modal: footer reachable, no Esc | fixed: skip, bar, main and footer `inert`, Tab/Shift+Tab wrap, Esc closes | `keyboard.log` D, `audit.json` |
| F4 priority by colour only | fixed: `.tcard-prio` words rendered ("Important" in the tree) | `extra.log` |
| F5 heading in summary | see A7 | |
| F6 filter tip not dismissible | fixed: Escape sets `display:none` | `keyboard.log` G |
| F7 `<title>` in body | re-confirmed (A11Y-18) | `audit.json` `titleInBody=true` |
| F8 refused modal without "Error:" title | fixed: `Error: Tasks - project-1 - iter` | `forms.log` 1 |
| F9 thin target margins | **now a failure** in the footer (A11Y-04) | axe |
| F10 hover-only action link | re-confirmed (A11Y-17) | read |
| F11 focused card without ring | re-confirmed (A11Y-12) | `keyboard.log` B |
| F12 accent on tint borderline | fine at 4.72 / 5.76; the muted-on-tint instances F12 said did not exist now do (A11Y-05) | run |
| F13 table semantics under 600 px | fixed: explicit `role=table/row/cell/columnheader` (`table.rs`) | read, `audit.json` |
| F14 nits | unchanged | |

### What is already good (verified by running)

- Structure: `lang="en"`, one `h1` on all 47 pages, no heading skips except A11Y-14, `main#main
  tabindex=-1` with a skip link that works and is not obscured, labelled `nav`s ("Main", "Sections",
  "Board sections", "Change day", "Change period", "Period", "Breadcrumb", "Notes"), labelled
  `aside`s, `role=search` on the filter form, every table with an `.sr` caption and explicit roles,
  zero unlabelled controls, zero duplicate ids, zero dangling `aria-*`/`for`/`list` references
  (`audit.json`).
- Pick mode is a full keyboard and pointer alternative to drag-and-drop: 30 cards, 30 action links
  ("Move/Schedule/Place <task>"), 24 "Move here · HH:00" slot buttons per day, 4 "Place here at
  <quadrant>" buttons, "Unschedule"/"Unplace" in the banner, `#pick` banner (`role=group`, named
  "Moving project-2/task-2 (now Mon 2 Nov, 10:00). Choose an hour.") takes focus on entry, the picked
  card says "Being moved" with `aria-current`, the result is announced in a `role=status` region and
  the title changes ("Moving project-1/task-6 - …"). Failed drops get `role=alert` (`board.js:133-145`).
- Pop-ups: first field autofocused, Tab wraps inside, Shift+Tab wraps backwards, Esc closes, ✕ is
  36×36 (44 on coarse pointers) named "Close", backdrop link is `tabindex=-1 aria-hidden`, the page
  behind is `inert` including the footer; the enhanced `<select>` is a proper `role=combobox` button
  (`aria-haspopup=listbox`, `aria-expanded`, `aria-controls`, `aria-activedescendant`, type-ahead,
  Home/End, Esc closes only the list; native select kept in sync); the `input[list]` combobox likewise.
- Forms: `label for` everywhere, hints wired with `aria-describedby`, refused New task / New project /
  edit forms focus a linked error box, mark the field `aria-invalid` and keep the input.
- Search palette: opens from the button or `/`, focus to the input, results announced ("42 results"),
  arrows move `aria-activedescendant`, Enter opens, Tab moves into the preview frame, Esc from the
  frame or the input closes and returns focus to the opener button.
- Filter bar: `role=search`, sr label, "?" button 26×26 with `aria-describedby` to the tip, Esc
  dismisses the tip, Enter filters, `role=status` count ("0 of 18 tasks match"), Clear link.
- Report: presets `aria-current="true"`, tab `page`, bad date → `role=alert`, arrows do not navigate
  while typing, Enter applies, Apply visible on focus.
- Motion: under `prefers-reduced-motion: reduce` every transition is 0 s, the modal and the breathing
  dot have `animation-name: none`, card lift and smooth scrolling are off; the dot otherwise breathes 3
  times only.
- Forced colors: cards and chips get solid `CanvasText` borders, the picked card and pick slots keep
  dashed Highlight borders, `.drop.over` gets a Highlight outline, the modal a 2 px border, the now
  line Highlight, tabs/nav/presets as above, focus rings 2 px solid.
- Focus is never hidden behind the sticky bar, pick banner, aside or panel (`obscured.log`; the
  "off-screen" counts on the report page are the smooth scroll still in flight when measured, and
  vanish under reduced motion).
- Colour: body, muted and accent text pass on their surfaces in both schemes; chip ink is computed and
  unit-tested; status text passes (WIP 4.72/5.76, done 5.33/7.97); error notice 13.8/5.9.
- Error pages (400, 404, unmatched URL): skip link, `main`, `h1`, plain sentence, link home; no axe
  violations; no reflow issue.

## Confidence

- High: axe counts and rules, computed contrast (token maths and Chrome's computed values agree to
  the last digit), reflow widths and culprits, target-size geometry, keyboard walks, form refusals,
  media emulation, structure audit (all `run`, repeatable with the scripts).
- Medium: the severity of A11Y-01 (the criterion is clear, the practical harm small); A11Y-08 (Chrome's
  tree only; other engines may keep `option`); A11Y-19 (same).
- Low: nothing listed as `inferred`.

## Out-of-scope observations (including security)

- Security: the bare `400` bodies echo the submitted name (`'Bad Name' is not a name: …`) as
  `text/plain`, so no injection, but they also echo the reason verbatim from `IterError`; if these are
  moved into HTML (A11Y-07) they must go through the view's escaping, which `error_box` does.
  `ui/notice.rs:43` writes an inline `<script>` with the notice id; a future CSP would need a nonce or
  the focus moved into `modal.js`. The search palette previews same-origin pages in an `<iframe>`
  (`search.js:152`), which the seed findings note needs `X-Frame-Options: SAMEORIGIN` rather than
  `DENY`. `aria-keyshortcuts="/"` plus no CSRF concerns observed in GET flows. Nothing new beyond the
  seed list.
- Design/consistency: the dialog close idiom differs (✕ + "Cancel" link in pop-ups, "Close" link on the
  step panel, "Back" elsewhere), as the seed findings say; the daily-note title uses "02/11" while
  the heading says "2 November 2026".
- Performance felt in a11y testing: the first paint of every page waits for `/search.js`, loaded
  synchronously before content (`frame.rs:47`); `defer` would do.
- Seed findings touched: "POST refusals answer bare-text 400" confirmed (A11Y-07); "`errors.rs` page
  lacks nav/search/footer" confirmed, rated Low (A11Y-21); "`/agents` and `/instructions` have no
  filter bar" is a design matter, not an a11y failure.

## What I could not assess

- Real screen readers (VoiceOver, NVDA, JAWS): live-region timing after a move, how the `summary`
  headings, the hidden date input and the palette options are actually spoken.
- Safari and Firefox (only Chrome 156; Playwright's Chromium bundle was incomplete on this machine):
  `inert`, `showPicker`, `content-visibility`, `clip-path` focus and date-input behaviour differ.
- `/agent/{id}/{n}` and the agent run/plan/step pages: the seeded stores have no runs; the New agent
  dialog was checked, not a running or finished agent.
- The share push flow past the preview (needs a platform), `POST /folders/pick`, wiki Accept/Revert
  pages with content.
- Text spacing override (1.4.12) and 200 % text-only zoom; print styles; Windows High Contrast beyond
  Chrome's `forced-colors` emulation; touch gestures and real mouse drag-and-drop (HTML5 DnD does not
  run under automation; the pick-mode alternative was exercised end to end instead).
- Colour-vision simulation of the priority edges (the words are present, so not needed for 1.4.1).
