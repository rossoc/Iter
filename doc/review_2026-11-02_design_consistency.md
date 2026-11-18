# Review 2026-11-02: design consistency (`iter serve`)

Lens: does the style a user learns on one page hold on every other page, judged from rendered pages.
Branch `feat/review`, binary `target/release/Iter` (HEAD). Paths are relative to `src/web/` unless they start with `doc/` or `design/`.

## Summary

The UI is one design: tokens only (no hex in a component sheet), a complete Tokyo Night dark theme, one focus ring, one tab style, one table template with its add row, one modal with the same ✕ on all seven pop-ups, and long names wrap everywhere. The inconsistencies are at the edges: the small screen, the ways out of a pop-up, the words on primary buttons, and the refusals that never reach the page.

Top 5:
1. **High, DC-01.** The top bar does not wrap: at 360px every page is 654px wide and scrolls sideways (nav 411px plus the Search button). The error pages, which have no nav, are the only pages that fit. `ui/frame.css:1,7`, `ui/search.css:2`.
2. **High, DC-02.** Project lists (Org Projects tab, Board Info) are 1099px wide at 360px: below 600px the path cell loses its `max-width:0` and its `white-space:nowrap` path stretches the table. `ui/project_list.css:5,17-18`.
3. **Medium, DC-04.** Adding a note, a session, an agent run or an instruction with a bad form answers a bare `text/plain` 400 ("bad request: a note needs a name"), lowercase and without a period, while New task, New project and the edit pages re-render with the error box. `doc/web_server.md` promises the re-render for all of them.
4. **Medium, DC-03.** Six ways to leave something: Cancel text link, ✕, `a.button` "Close", `a.button` "Back", `a.button.small` "Cancel", an underlined muted "Close". Re-confirmed from the October review (H1), with two new variants since.
5. **Medium, DC-05.** Nine primary-button labels for "do it": Save, Create task, Create project, Create, Add session, Add note, an icon-only ▶ (Run now), Copy, Push; and the Share pop-up shows "Copy" twice.

Findings: 2 High, 10 Medium, 8 Low. No Blocker.

## Scope

Every route in the brief's route map that renders a page, on the shared scale-4 store (`:3999`) and on an empty store I served (`:4010`): Home, Settings, Organization (Info, Tasks, Tasks + New task, Report, Report + Share, Settings/edit, New project, daily note Info and Edit), Project (Info, Tasks, Tasks + New task, Settings/edit, daily note, a project without organization), Task (Info, a task with an 81-character name, Sessions, Sessions + New session, Notes, Notes + New note, Agents, Agents + New agent, Wiki, Settings/edit), Agents, Instructions, Instructions + New instruction, Instruction page, Boards, Board (Agenda, Agenda in pick mode, Agenda `?moved=`, Matrix, Matrix in pick mode, Info, Info + New project, Settings/edit), the 400 and 404 pages and `/nope`, and the empty-store Home, Boards, Agents, Instructions, Settings and Instruction page. 49 pages × 2 widths × 2 themes = 196 screenshots, plus 3 for focus and the search palette.

Not covered: `/agent/{id}/{n}` (the seed has no agent runs and starting one launches a real agent), the folder browser `/folders`, the push preview pop-up, the plan step aside (`?step=`), hover-only states, drag and drop.

## Method

- Read `doc/review_tools/BRIEF.md`, `SEED_FINDINGS.md`, `doc/web.md`, `doc/web_components.md`, `doc/web_server.md`, `design/README.md`, `design/04-direction-for-iter.md`, `design/05-svg-icons.md`, the October `doc/review_design_consistency.md`, and the sources named in the findings (`ui/button.rs`, `ui/modal.rs`, `ui/form.rs`, `ui/pick_bar.rs`, `ui/page_header.rs`, `ui/empty_*.rs`, `errors.rs`, `ui/frame.css`, `ui/project_list.css`, `ui/button.css`, `ui/tabs.css`, `org.css`, `org_report.rs`, `org_share.rs`, `task_runs.rs`, `task_agent.rs`, `instructions/page.rs`, `instructions/save.rs`, `task_notes.rs`, `task_sessions.rs`, `task_new.rs`, `sections.rs`, `daily_notes.rs`).
- Empty store: `XDG_CONFIG_HOME=$SCRATCH/design-empty ITER_PLATFORM_KEY=dummy $BIN serve -p 4010 --no-open`. For the POST tests I then inserted one organization, project, task and board into that store with `sqlite3` (the server opens the file per request, so no restart).
- Screenshots: `doc/review_tools/design_consistency/shoot.mjs` (puppeteer-core 23.11.1 driving the installed Chrome, `npm install` in that folder; routes in `routes.json`). Each route at 1280×900 and 360×800, full page, light and dark. **Dark is the site's own theme**: `page.emulateMediaFeatures([{name:'prefers-color-scheme', value:'dark'}])`, which renders the Tokyo Night tokens (verified: body `#24283b`, orange accent). The script also records DOM facts per page (h1, kicker, tabs, buttons, dialog exits, empty-state text, footer, inline styles, control counts, computed font sizes, `scrollWidth - innerWidth`) into `inventory.json`. Screenshots are in `doc/review_tools/design_consistency/screens/<route>_<width>_<theme>.png`; I looked at 20 of them (home, org tasks, New task, Share, New agent, agenda pick, matrix pick, report, long task, wiki, daily note, instruction, 404, empty home, board info, agenda 360, report 360, matrix 360, New session 360 dark, focus, palette).
- `overflow.mjs`: lists the elements whose right edge passes a 360px viewport. `focus.mjs`: tabs to the first tab link and screenshots the ring, then opens the palette with `/`.
- POST refusals: `curl -X POST --data ...` with empty or invalid fields against `:4010` (commands in DC-04).
- Not checked: hover animations, reduced motion, forced colors, print, a real touch device, screen readers (other lenses).

## Inventory (rendered, 1280px light)

Kicker and tab facts come from `inventory.json` and the source; "ctrl" is the number of links, buttons and fields inside `main` (the bar and footer add 41 on the seeded store).

| Page | Kicker | h1 | Header actions | Tabs (aria-label) | Empty wording seen | Add / edit / dismiss | Filter bar | Primary label | ctrl |
|---|---|---|---|---|---|---|---|---|---|
| `/` | eyebrow "Home" | "Where to next?" + lede | none | none | empty store: dashed box "No projects yet. Create one with `iter init` or `iter new <path>`." | none | no | none | 28 |
| `/settings` | eyebrow "Global" | "Settings" | none | none | n/a | Save + Cancel link (to `/`) | no | Save | 12 |
| `/org/1` Info | eyebrow "Organization" | name | none (edit is a tab) | Sections: Info, Projects n, Tasks n, Notes n, Report, Settings | "No projects yet." (tr.no-rows) | "+ New project…" add row → pop-up | yes (Projects) | none | 7 |
| `/org/1?tab=tasks` | same | name | none | same | "No tasks yet." / "No tasks match." | "+ New task…" add row → pop-up | yes | none | 47 |
| `…&new=task` | same | name | none | same | n/a | modal: ✕ "Close" + Cancel link + backdrop; 9 fields, inner scroll | yes | Create task | 46 (+ modal) |
| `/org/1?tab=report` | same | name | none | same | "No sessions in this period." | prev/next `a.button`, presets pill (Today, 7 days, 30 days, All time), two date inputs, hidden Apply, Share `a.button` | no | none (Share is secondary) | 19 |
| `…&share=1` | same | name | none | same | n/a | modal: ✕ + Cancel link; modality toggle Copy/Push styled as bordered buttons; two `.button.primary` (Copy, Push) | no | Copy, Push | 19 (+ modal) |
| `/org/1/edit` | same | name (title "Settings - org-1") | none | same, Settings current | "No projects yet." in checklist | Save + Cancel link | no | Save | 41 |
| `/org/1/notes/{date}` | crumbs org / Notes / 2 November 2026 | note title ("02/11") | none | Note sections: Info, Edit | "Nothing written on this day." | Edit tab, Save + Cancel | no | Save | 5 / 7 |
| `/project/1` | crumbs org / Project | name | none | Sections: Info, Tasks n, Notes n, Settings | — | — | no | none | 5 |
| `/project/10` (no org) | eyebrow "Project" | name | none | same | — | — | no | none | 4 |
| `/project/1?tab=tasks` (+new) | crumbs | name | none | same | "No tasks yet." | add row → pop-up; ✕ + Cancel | yes | Create task | 16 |
| `/project/1/edit` | crumbs | name | none | same, Settings current | n/a | Save + Cancel; two folder buttons | no | Save | 21 |
| `/task/1` | crumbs org / project / Task | name | none | Sections: Info, Sessions n, Agents n, Notes, Wiki, Settings | task 105: blank left column (empty description) | — | no | none | 8 |
| `?tab=sessions` (+new) | crumbs | name | none | same | "No sessions yet." | add row; modal ✕ + Cancel; 3 fields | no | Add session | 9 |
| `?tab=notes` (+new) | crumbs | name | none | same (title "Notes") | "No notes yet." | add row; modal ✕ + Cancel | no | Add note | 9 |
| `?tab=agent` (+new) | crumbs | name | none | same (tab "Agents", title "Agent") | "No agent has worked on this task yet." | add row; modal ✕ + `a.button` "Close"; no "* Required" | no | ▶ icon only (Run now) + Queue | 9 |
| `?tab=wiki` | crumbs | name | none | same | "No sketches yet."; a git hint with an absolute path in an `.empty-line` | Accept / Throw away post buttons (when pending) | no | none | 8 |
| `/task/1/edit` | crumbs | name | none | same, Settings current | n/a | Save + Cancel | no | Save | 21 |
| `/agents` | eyebrow "Global" | "Agents" + lede | none | none | "No agent has worked on any task yet." | none | **no** | none | 0 |
| `/instructions` (+new) | eyebrow "Global" | "Instructions" | none | none | — | add row; modal ✕ + Cancel | **no** | Create | 6 |
| `/instruction/plan` | crumbs Instructions / Plan | "Plan" | none | Sections: Info, Agents n, Settings | "No agent has been given this instruction yet." | Settings tab: Save, "Use the inherited one", `a.button` **Back** | no | Save | 4 |
| `/boards` | eyebrow "Board" | "Boards" + lede | none | none | empty: dashed box "No boards yet. Create one with `iter board new`." | none | no | none | 16 |
| `/board/1` agenda | crumbs Boards / Agenda | name | none | Board sections: Agenda, Eisenhower matrix, Info, Settings | "No important tasks waiting." / "No urgent tasks waiting." | day nav `a.button` ×3, fold button, card "Schedule"/"Move" links; pick bar: Unschedule + **Cancel `a.button.small`**, 24 "Move here · HH:00" slots | no | none | 84 / 110 |
| `/board/1/matrix` | crumbs Boards / Eisenhower matrix | name | none | same | "Nothing to do first." "Nothing to plan." "Nothing to delegate." | pick bar Cancel; "Place here" `button.small` ×4 | no | none | 79 / 84 |
| `/board/1/info` (+new) | crumbs Boards / Info | name | none | same | "No projects yet." | add row → pop-up; ✕ + Cancel; folder button | yes | Create project | 16 |
| `/board/1/edit` | crumbs Boards / Settings | name | none | **"Sections"** (not "Board sections") | "No projects yet." | Save + Cancel | no | Save | 29 |
| 400 / 404 / `/nope` | none | "Bad request" / "Not found" | none | none | "This address is not valid." / "Nothing lives at this address." | "Back to the home page" link | no | none | 1 (no nav, no search, no footer) |

Footer: on every page but the error pages. Search button: every page but the error pages. Inline `style=`: 7 sites, all data-driven (tag colours, `--at`, `--urgent`/`--important`, share-bar widths). Computed font sizes seen across pages: 11, 11.05, 11.9, 12, 12.75, 13, 13.33, 13.6, 14, 15, 16, 17, 20, 22, 40, 48px.

## Findings

| ID | Severity | Where | Evidence | Effort | One-line fix |
|---|---|---|---|---|---|
| DC-01 | High | `ui/frame.css:1,7`, `ui/search.css:2` | run | S | Let `.bar` wrap or drop nav labels below 600px |
| DC-02 | High | `ui/project_list.css:5,17-18` | run | S | Keep `max-width:0`/`min-width:0` on `.path-cell` below 600px or let the path wrap |
| DC-03 | Medium | `ui/form.rs:22`, `ui/modal.rs:27`, `ui/pick_bar.rs:38`, `task_runs.rs:310`, `instructions/page.rs:90`, `task_agent.rs:285`, `ui/steps.css:11` | run | S | One rule: Cancel is the `.u` link in forms and pop-ups, ✕ only on modals; drop "Back"/"Close"/`.step-close` |
| DC-04 | Medium | `task_notes.rs:94`, `task_sessions.rs:81`, `instructions/save.rs:65-90`, `task_runs.rs:410,473` | run | M | Re-render the pop-up with a `FormError` (as `task_new.rs:177-230` does) |
| DC-05 | Medium | `ui/form.rs:21`, `task_new.rs`, `project_new.rs`, `instructions/list.rs`, `task_sessions.rs`, `task_notes.rs`, `task_runs.rs:305`, `agents.rs:271`, `org_share.rs:367-371` | run | S | Pick "Create X" or "Add X" for every New pop-up; give Run now a visible label |
| DC-06 | Medium | `org_share.rs:367-372`, `org.css:30-40` | run | S | Make the modality toggle look like a segmented control (it reuses `.presets`-like pills), one primary submit |
| DC-07 | Medium | `sections.rs:84,172,199`, `edit_page.rs:87` vs `board_header.rs:70`, `sections.rs:147` vs `:166` | run | S | Note tab "Edit" → "Settings"? (or the reverse everywhere); one `aria-label`; tab "Agents" and title "Agent" agree |
| DC-08 | Medium | `instructions/page.rs:81,133`, `agents.rs:220`, `task_wiki.rs:125,148`, `task_runs.rs:287`, `task_agent.rs:251,370` | run | S | `.empty-line` only for "nothing here"; notes and errors through `notice`; 33 wordings → list the allowed set in `doc/web.md` |
| DC-09 | Medium | `agents.rs:82-100`, `instructions/list.rs:45-70` | run | M | Give the two global lists the `filter_bar` the task and project lists have |
| DC-10 | Medium | `errors.rs:14-17` | run | S | Build the 400/404 through `frame` (nav, search, footer) |
| DC-11 | Medium | `boards.rs:52`, `org.rs:226`, `project.rs:142`, `settings.rs:152`, `agents.rs:82`, `instructions/list.rs:45`, `ui/frame.rs` nav | run | S | Eyebrow matches the h1 ("Boards"), crumbs for every entity page incl. Organization, nav "Boards" |
| DC-12 | Medium | `task_new.rs`, `task_fields.rs`, `ui/modal.css:5` | run | M | New task pop-up: Project, Name, Description only; the rest on the Settings tab |
| DC-13 | Low | `daily_notes.rs:123,257,334`, `agenda.rs:122,144`, `agenda_cards.rs:53`, `org_report.rs` (ISO), native date inputs | run | S | Two formats: short `%a %-d %b` in lists, long `%A %-d %B %Y` in headings; ISO only in mono tables |
| DC-14 | Low | `ui/*.css` (25 `font-size` literals), `design/04` §1 | run | M | Four `--t-*` tokens; stop `em`-on-`em` nesting that yields 11.05/11.9/12.75/13.33/13.6px |
| DC-15 | Low | `design/README.md:8-10`, `design/04:3-5`, `doc/web.md:4,14,16-18,22,30,36-38,40`, `doc/web_final_review.md:24,57,59`, `doc/web_cleanup.md:26,30,32` | read | S | `/v2.css` → `/style.css`, `src/web/v2.css` → `base.css`; "Edit button"/"Edit {name}" → Settings tab; task page has tabs now |
| DC-16 | Low | `ui/frame.rs:32-33` | run | S | Render `<title>` in `<head>` (errors.rs already does) |
| DC-17 | Low | `org_share.rs:367,371,415`, `instructions/page.rs:85-90`, `agents.rs:271-273`, `task_runs.rs:182,305-310`, `org_report.rs:174-194,253`, `ui/day_nav.rs:53-55`, `ui/side_rail.rs:25`, `ui/field.rs:145` | read | S | A `button(label, kind)` / `link_button` component in `ui/button.rs`; `post_button` exists for the POST case |
| DC-18 | Low | `task.rs` (Info body), screenshot `task_long_1280_light.png` | run | S | "No description." `empty_line` when the description is empty |
| DC-19 | Low | `task_wiki.rs:120-130`, `task_wiki.rs:148` | run | S | Shorten the git hint, drop the absolute path, give "No sketches yet." its section label |
| DC-20 | Low | `ui/notice.rs:37`, board edit refusal | run | S | Same `role="alert"` on every refused form (board edit's error box had none) |

### DC-01 · design_consistency · High · `ui/frame.css:1,7`, `ui/search.css:2` · run · S · let the bar wrap or hide the nav labels below 600px
`.bar{display:flex;…;height:var(--bar-h)}` with `.bar nav{display:flex;gap:4px}` and `.search-btn{margin-left:auto}` never wraps. At a 360px viewport `document.scrollingElement.scrollWidth` is 654 on all 46 framed pages (`inventory.json`, `overflowX: 294`) and 360 on the three error pages. `overflow.mjs` names the culprits: `NAV right=513 w=411`, `BUTTON.search-btn right=654 w=117`. In the 360px screenshots the page content sits in the left 55% and the footer stops short, which is what a phone user sees after scrolling sideways. `doc/web_components.md` (Accessibility) promises "no horizontal scroll at 320px"; `doc/web_final_review.md:91` says 320px screenshots were compared, so this regressed when Agents and Instructions joined the nav. Repro: `node doc/review_tools/design_consistency/overflow.mjs http://127.0.0.1:3999/`.

### DC-02 · design_consistency · High · `ui/project_list.css:5,17-18` · run · S · keep the path cell shrinkable below 600px
`.path{display:block;overflow:hidden;white-space:nowrap;text-overflow:ellipsis}` relies on `td.path-cell{width:100%;max-width:0}` to have a width to clip to. The `@media (max-width:600px)` rule replaces it with `flex:1 1 0;width:auto;max-width:none;min-width:0`, but the row is still a table row there (`table.css:37` only makes `.add`, `.section` and `.no-rows` rows blocks), so the nowrap path sets the column width: `TABLE.tbl projects right=1119 w=1099`, `TD.path-cell w=1018`. Seen on `/board/1/info` and `/org/1?tab=projects` at 360px (`board_info_360_light.png`: the list runs off the right edge while the path itself is invisible because the hover rule hides it). Repro: `node overflow.mjs "http://127.0.0.1:3999/org/1?tab=projects"`.

### DC-03 · Medium · re-confirmed (Oct H1), now six idioms · run
Seen rendered: (1) Cancel as `a.u` under Save in every `form_shell` form and in the New task/session/note/project/instruction pop-ups (`ui/form.rs:22`) and in Share (`org_share.rs:372,418`); (2) the modal ✕ with hidden text "Close" (`ui/modal.rs:27`); (3) `a.button` "Close" in the New agent pop-up next to the ✕ (`task_runs.rs:310`, `task_agent_new_1280_light.png`: two exits of two shapes in one box); (4) `a.button` "Back" on the Instruction Settings tab (`instructions/page.rs:90`); (5) `a.button.small` "Cancel" in the pick bar (`ui/pick_bar.rs:38`, `board_agenda_pick_1280_light.png`); (6) `.step-close`, a muted underlined "Close" (`task_agent.rs:285`, `ui/steps.css:11`). The October fix proposal stands.

### DC-04 · Medium · `task_notes.rs:94`, `task_sessions.rs:81`, `instructions/save.rs:65-90`, `task_runs.rs:410,473` · run · M
Against my `:4010` store (`sqlite3` seeded one task), every refusal below is `400 text/plain`, lowercase, no period, and leaves the user on a blank browser page instead of the pop-up with the error:
```
curl -X POST http://127.0.0.1:4010/task/1/notes    --data 'name=&text='                 → bad request: a note needs a name
curl -X POST http://127.0.0.1:4010/task/1/sessions --data 'start=&end=&message='        → bad request: '' is not a date and time
curl -X POST http://127.0.0.1:4010/task/1/sessions --data 'start=2026-11-02T10:00&end=2026-11-02T09:00' → bad request: a session must end after it starts
curl -X POST http://127.0.0.1:4010/instructions    --data 'name=Bad%20Name&…'           → bad request: 'Bad Name' is not a name: lowercase letters, digits and dashes
curl -X POST http://127.0.0.1:4010/task/1/runs     --data 'instruction=&agent=claude&…' → bad request: '' is not an instruction
```
Whereas `POST /task/new` and `POST /org/1/project/quick` with an empty name return `200 text/html` with `<title>Error: Tasks - p1 - iter</title>`, `role="dialog"` and `id="form-error"` (the seed's claim that New task answers bare 400 is **refuted**; its 400 is only for a non-numeric `project`). `POST /org/1/edit` and `/board/1/edit` re-render with "Error: Organization name cannot be empty." `POST /org/1/share` with `format=md` renders the report page (200) with `unknown format 'md'` in the notice: consistent with the pop-up idiom but raw and lowercase. `doc/web.md` (Messages) says refusals are "capitalised sentences with a period" and `doc/web_server.md` (Tabs and pop-ups) says these POSTs "re-render the pop-up with the typed values and the error on refusal". Both are untrue for notes, sessions, runs and instructions.

### DC-05 · Medium · primary labels · run
Rendered primaries: Save (Settings, four Settings tabs, note Edit, instruction Settings), Create task, Create project, Create (instruction), Add session, Add note, ▶ with only `aria-label="Run now"` (New agent, `task_runs.rs:305`, `agents.rs:271`), Copy and Push (Share). "Create" vs "Add" splits by entity for no reason a user can see, and the agent pop-up is the only place where the primary action has no visible word (`task_agent_new_1280_light.png`), next to two secondaries that do (Queue, Close). The same compose form also lacks the "* Required" note every other form has because it is not built through `form_shell`.

### DC-06 · Medium · `org_share.rs:355-372`, `org.css:30-40` · run
`org_report_share_1280_dark.png`: the Copy / Push modality toggle (radio labels styled `.share-modes .mode`, accent border when checked) sits above a primary "Copy" button of the same word, so the dialog reads as two Copy buttons; the Push label is a bordered button too. The site's other segmented controls are the nav pills and the report presets (`.presets`); the share toggle is a third shape.

### DC-07 · Medium · tabs and titles · run
`sections.rs:84,172` and `board_header.rs` call the edit page "Settings" on organization, project, task, board and instruction; the daily note calls its editor "Edit" (`sections.rs:199`). The board's edit page labels its tab row `aria-label="Sections"` (`edit_page.rs:87`) while the other three board pages say "Board sections" (`board_header.rs:70`); a screen-reader user hears two different landmarks for the same row. The task's tab says "Agents" (`sections.rs:166`) and its document title says "Agent - task-1" (`sections.rs:147`). Each is small; together they are the tab system not agreeing with itself.

### DC-08 · Medium · empty states · run
33 distinct `"No …"`/`"Nothing …"` strings in `src/web` (list in Method's grep; the seed's "~35" is confirmed to within two). The documented rule (`design/04` §3, `doc/web.md` Messages) covers "No X yet." and the matrix/agenda/report exceptions; not covered are "No tasks match." / "No projects match." (a real filter, fine), "No unfinished tasks yet. Add some…", "Nothing written on this day.", "Nothing printed yet.", "No prompt was written.", "Nothing pending: the wiki is as it was last committed.", "No agent has worked on this task yet." vs "…on any task yet." vs "No agent has been given this instruction yet.". Eight `<p class="empty-line">` are written by hand (`instructions/page.rs:81,133`, `agents.rs:220`, `task_wiki.rs:125,148`, `task_runs.rs:287`, `task_agent.rs:251,370`) and six of them carry a note or an error ("The front matter cannot be read: …", "The prompt could not be made: …", "The start of the log is not shown."), so the muted "nothing here" style is doing three jobs.

### DC-09 · Medium · `/agents`, `/instructions` · run
Both are `data_table`s like the task and project lists but have no `filter_bar` (`inventory.json`: `filterBar:false`, `controls.total` 0 and 6). The agents list is "of every task, the latest first" and will be the longest list in the app; the instruction list gains a pop-up add row but no search. Confirms the seed.

### DC-10 · Medium · `errors.rs:14-17` · run
`err404_1280_light.png`: brand link only in the bar, no nav, no Search, no footer, no eyebrow; `body` background and type match. `/nope` now gets this page too (fixed since Oct L5, which saw bare text). It is the one page a user reaches by a typo and the one page that does not offer the rest of the site.

### DC-11 · Medium · kicker rule · run · re-confirmed (Oct H3)
Eyebrow "Board" over h1 "Boards" and nav "Board" linking `/boards` (`boards.rs:52`); eyebrow "Global" over Settings, Agents and Instructions; eyebrow "Organization" on the org page while project and task use crumbs; a project without an organization falls back to eyebrow "Project" (`project.rs:142`, `project_noorg_1280_light.png`). The folder-picker kicker from October is gone (the picker is a pop-up now), so this is the remaining half of H3.

### DC-12 · Medium · New task pop-up · run · re-confirmed (Oct M3)
`org_tasks_new_1280_light.png`: Project, Name, Description, Status, Tags, Schedule (Start, Duration), GitHub (issue, branch prefix): the box hits `max-height:calc(100dvh - 32px)` and scrolls inside at 900px high; Create task and Cancel are below the fold. New session has three fields and New note two.

### DC-13 to DC-20 (Low)
- **DC-13 dates.** On one daily-note page: crumb "2 November 2026" (`%-d %B %Y`), body "Monday 2 November 2026", h1 the file's title ("02/11" on the org note, "today" on the project note), tab list "Sun 1 Nov"; cards "Sun 1 Nov"; report tables ISO `2026-11-01`; the From/To inputs `27/10/2026` (native). Six shapes for a date.
- **DC-14 type scale.** 16 computed sizes on the pages (list in Inventory); `design/04` §1 already records "not built". The fractional sizes come from `em` sizes inside `rem`-sized parents (code in prose, counts in labels).
- **DC-15 stale docs.** `design/README.md` and `design/04` still say the sheet is `/v2.css` from `src/web/v2.css`; it is `/style.css` from `base.css` (`ui/mod.rs:124`). `doc/web.md:4` says "`/style.css` were removed" (it is the current sheet). `doc/web.md` describes an "Edit button" and edit pages with crumb "… / Edit" and h1 "Edit organization"; rendered, edit is a "Settings" tab, the h1 is the entity's name and the title "Settings - org-1" (`inventory.json`). `doc/web.md:36-38` says the task page has "Left: description, then Sessions table"; it has six tabs. `doc/web_final_review.md:24` (A6b) and `:57,59` cite `/v2.css`; A6b is otherwise accurate (the error page has title, skip link, one main, one h1).
- **DC-16 `<title>` in `<body>`.** `curl -s http://127.0.0.1:3999/ | head -c 400` shows `</head><body><title>iter</title>`; `ui/frame.rs:32-33` explains why. `errors.rs:15` puts it in `<head>`. Browsers cope; validators and some crawlers do not.
- **DC-17 hand-written buttons.** `class="button…"` is spelled out in 8 page files (list in the table), including three copies of the Run/Save/Queue/Close row (`agents.rs:271-273`, `task_runs.rs:305-310`) and the report's prev/next pair that duplicates `ui/day_nav.rs`. Confirms the seed; `add_button` from October is gone.
- **DC-18 blank Info.** `task_long_1280_light.png`: task 105 has no description, so the Info tab is a Details panel on the right and nothing on the left.
- **DC-19 wiki tab.** `task_wiki_1280_light.png`: an absolute `/private/tmp/…/notes/org-1/project-1` path and "Run `git init` in it" in an empty-line under "PENDING CHANGES 0"; "No sketches yet." floats with no section label.
- **DC-20 error box role.** Org edit's refusal has `role="alert"`, board edit's does not (curl output in DC-04); `ui/notice.rs:37` adds it only when `error` is set, so one caller passes `false`.

## What is consistent and good (seen rendered)
- Tokens throughout: zero hex/rgb literals in `ui/*.css` and `org.css`; the dark theme is complete on all 49 pages (no unstyled control, native date and select follow `color-scheme`).
- One focus ring (`focus_tasks_1280_light.png`: 2px accent, offset, rounded) on tabs, buttons, links and the palette.
- One tab style with the sliding underline, one count badge, one status chip (icon + word) in tables, panels, cards and the report.
- One `data_table` with the tinted "+ New …" add row and `tr.no-rows` empty row on tasks, projects, sessions, notes, agents, instructions.
- All seven pop-ups share the same box, heading size, ✕ and backdrop; `inert` behind them works (the blurred page in every modal screenshot).
- Long names wrap everywhere: the 81-character task name in the h1 (`text-wrap:balance`), table rows, cards, the Overdue list; the agenda, matrix, report, forms and the palette all fit 360px once the bar is fixed (`overflow.mjs` lists only the bar on those pages).
- Inline `style=` is confined to data (tag colours, now-line, priority colours, share bars), never layout.
- The footer is the same on every framed page and folds to one column at 360px.

## Confidence
High for DC-01, DC-02, DC-04, DC-10, DC-16 (measured or curl'd). High for DC-03, DC-05, DC-06, DC-07, DC-08, DC-09, DC-11, DC-12, DC-18, DC-19 (seen in screenshots, confirmed in source). Medium for DC-13, DC-14, DC-17 (counts from source and computed styles; how much a user minds is judgement). DC-15 is a doc read. DC-20 was seen once in curl output and not traced to its caller.

## Out-of-scope observations
- Security: the wiki tab prints an absolute server path (DC-19); the share refusal echoes the submitted format raw (`unknown format 'md'`, `org_share.rs`); the bare 400 bodies echo user input (`'Bad Name' is not a name`) but as `text/plain`, so not injectable. The Report tab's two date inputs auto-submit through `org.js`; without script the Apply button is visually hidden until focused (`org.css:21-22`), which is fine but undocumented in `doc/web.md`.
- Accessibility: DC-20 (missing `role="alert"`), DC-07 (two `aria-label`s for one tab row), DC-05 (icon-only primary), DC-01 (horizontal scroll fails the reflow criterion at 320/360px).
- Performance: every pop-up renders the full page behind it (the New task pop-up page has 102 interactive elements), and the footer's three queries run on every page.

## What I could not assess
- `/agent/{id}/{n}` and the plan-step aside: no runs in the seed and starting one launches an agent.
- Hover-only affordances (card Schedule/Move links, project paths) and drag and drop; reduced motion and forced colours.
- Whether the modality toggle in Share (DC-06) is clearer with a pointer than in a still.
- Real touch targets on a device; the 44px rule was not measured.
