# Review: design consistency

Branch `feat/html-refactor`. All paths are relative to `src/web/` unless they start with `design/`.

## Scope

The lens is whether the style a user learns on the first page holds on every other page, without overloading any of them.

Pages covered:
- Home, Boards, Board (Agenda, Matrix, Info), Organization (Info, Tasks, Report), Project (Info, Tasks) and Task.
- Settings, the folder picker and the error pages.
- The edit forms for organization, project, task and board.
- The New task and New project pop-ups.

Source covered: `src/web/**`, `forms/**`, `board.js`, `assets.rs` and `design/*.md`.

I treated `design/04-direction-for-iter.md` as the intent document. I did not edit any source.

## Method

What I did:
- Read the shared components in `ui/` (header, crumbs, buttons, form actions, notices, empty states, modal, tabs, pick bar, task card, panels, group card, day nav).
- Read each page's view code, and the CSS next to each component and `v2.css`.
- Compared what I found against `design/04`. I grepped the CSS for literal colors and tallied font sizes.
- **Ran the app (verified by running).** I used the existing `target/debug/iter serve` binary on port 3917 with an empty, isolated `HOME`. Fetched with curl: `/`, `/boards`, `/settings`, `/nope`, `/org/1` and `/board/1`. This confirmed the empty states, the footer, the error pages and the Settings form text.
- Everything else is **inferred from reading source**. That includes all populated pages, the modals, the pick mode and every interaction.

What I did not check:
- Visual rendering. I took no screenshots and did not check the light and dark themes, spacing, alignment or contrast.
- The dark theme and narrow-screen layouts.
- Keyboard and screen-reader behaviour, drag-and-drop in `board.js` beyond its header contract, and the org report beyond its markup.
- Whether the debug binary matches the current source. It is newer than most files, but I did not rebuild it.
- The `doc/web_*.md` files, which I treated as context only.

## Inventory

| Page | Kicker (above h1) | Header actions | Tabs | Empty state | Add pattern |
|---|---|---|---|---|---|
| Home `/` | Eyebrow "Home", h1 "Where to next?", lede | none | none | dashed box, FOLDER icon: "No projects yet. Create one with `iter init`…" (home.rs:57) | none (CLI hint) |
| Boards | Eyebrow "**Board**", h1 "Boards", lede | none | none | dashed box, KANBAN icon: "No boards yet. Create one with `iter board new`" (boards.rs:56) | none (CLI hint) |
| Board Agenda, Matrix, Info | Crumbs "Boards / {tab}", h1 = board name | Edit button (board_header.rs:67) | Agenda, Matrix ("Eisenhower matrix"), Info; label "Board sections" | box "No unfinished tasks yet…" (board_empty.rs); per-list `empty_line` | none for tasks. Info has a "New project…" row. |
| Organization | Eyebrow "Organization" (org.rs:191) | Edit button | Info, Tasks (count), Report; label "Sections" | `empty_line` "No tasks yet." / "No projects yet." | "New task…" row, "New project…" row, both opening modals |
| Project | Crumbs "{org} / Project", or Eyebrow "Project" when there is no org (project.rs:133) | Edit button | Info, Tasks (count); label "Sections" | `empty_line` | "New task…" row |
| Task | Crumbs "{org} / {project} / Task" | Edit button | none | `empty_line` "No sessions yet." | none |
| Settings | Eyebrow "Global" | none | none | n/a | n/a |
| Folder picker | Eyebrow "New project" | none | none | `folder_list` none text | n/a |
| Edit org, project, task, board | Crumbs "… / {name} / Edit", h1 "Edit {name}" | none | none | n/a | n/a |
| New task, New project | Modal (h2 "New task" / "New project") over the list | ✕ close | none | n/a | n/a |
| Error 404/400 | none, own bare document (errors.rs:19) | none | none | n/a | n/a |

The Settings page ends with `form_actions(cancel: "/")`, so Cancel goes to Home.

Other patterns used across pages:
- **Primary button.** `.button.primary` for submit, in `form_actions` and "Use this folder" (folders.rs:173).
- **Secondary button.** `.button` for Edit, Choose…, Prev/Today/Next and Apply.
- **Link-style cancel.** Cancel in forms is a `.u` link (form_actions.rs:16).
- **Pick bar.** Cancel is a `.button.small` (pick_bar.rs:43).
- **Row-as-link add.** "New task…" and "New project…" are dashed first rows with `PLUS` (task_table.rs:63, project_list.rs:42).
- **Unused add button.** `add_button` in `ui/button.rs:22` is defined but never used (verified by grep).
- **Colors.** None are hard-coded in `ui/*.css` or `org.css` (grep found zero hex or rgb values). Tokens are used cleanly.
- **Font sizes.** Literals only: `.8125rem` ×17, `.875rem` ×15, `1.0625rem` ×4, and others. The design doc admits there is no type scale.

## Findings

### High

**H1. Three different "close or cancel" idioms.**
- A form's Cancel is a text link (`ui/form_actions.rs:16`).
- The pick bar's Cancel is a bordered small button (`ui/pick_bar.rs:43`).
- A modal has three exits: the ✕ icon, the form's Cancel, and the backdrop (`ui/modal.rs:25-27`).
- The user has to relearn what "dismiss" looks like in each context.
- Fix:
  - Pick a rule. Cancel in a form or modal is always a `.u` link, and the pick bar's Cancel changes to match, or the other way round.
  - Keep one ✕ for modals.
  - Document the rule in `design/04` section 3 (Buttons).

**H2. The tab order and the default tab differ between sections.**
- Organization and project use Info, Tasks, Report, with Info first and the default (`sections.rs:77-87`).
- Board uses Agenda, Matrix, Info, with Info last and Agenda as the default (`board_header.rs:56-63`).
- The same word "Info" sits at opposite ends of the row.
- Fix: put Info in the same position everywhere, or rename the board's tab (for example "About").
- Fix: land on the work tab by default.
  - A project's Info opens first, even though most visits are for its tasks.
  - The task-count badge only appears on the tab you are not on.

**H3. The kicker (the line above the h1) follows no single rule.**
- Home has "Home", Boards has "Board" (singular, while the h1 is "Boards"), Settings has "Global", and the folder picker has "New project".
- An organization has the eyebrow "Organization", but a project with an organization has crumbs "{org} / Project".
- A project with no organization falls back to the eyebrow "Project" (`project.rs:133`).
- The task page and the board pages use crumbs.
- The rule is not visible to the user. It depends on whether a parent exists.
- Fix:
  - Always use crumbs for entity pages, with the root crumb "Organizations", "Boards" and so on.
  - Keep eyebrows only for top-level singletons, and make them match the h1. "Board" over "Boards" reads as a typo.
  - Change the folder picker's kicker to a crumb trail ("New project / Choose a folder").

### Medium

**M1. "Add" has two forms and a third is dead code.**
- Edit is a header button (`ui/button.rs:12`).
- Add task and add project are first table rows with an ellipsis ("New task…", `ui/task_table.rs:63`).
- `add_button` (`ui/button.rs:22`) exists as the shared "+" component and is unused.
- A user who learns "actions are top right" will not find New task or New project.
- Fix: delete `add_button`, or move the add action into `head-actions`. Either way, document it.

**M2. Creating is a pop-up, editing is a page.**
- New task and New project are modals (`task_new.rs:148`, `project_new.rs:174`).
- Edit task and Edit project are full pages with crumbs (`edit_page.rs:85`).
- The folder picker (`folders.rs:167`) then leaves the modal for a full page and returns to the modal. This is a third surface in one flow.
- Fix:
  - Document the rule, "create from a list is a modal, edit is a page", or unify.
  - Add a way back from the folder picker that reads as "back to New project". Its Cancel is the same word as a close.

**M3. The New task and New project pop-ups carry the full edit form.**
- New task shows 8 fields: Project, Name, Description, Status, Tags, Start, Duration, GitHub issue and Branch prefix (`task_new.rs:162-166` plus `task_fields.rs`).
- New project shows Name, Description, Base path and 6 settings (`project_new.rs:181-188` plus `settings_fields.rs`).
- Fix: show only Name, Description and Base path in the pop-up. The rest are already editable on the edit page and are mostly "Leave empty for none".

**M4. Edit and New forms are not alike.**
- New project has a "Choose…" folder button next to Base path (`project_new.rs:185`). Edit project does not (`project_edit.rs`, Base path field).
- The submit label is "Create task" and "Create project", but "Save" in edit forms and Settings.
- Fix: share the Base path field, including Choose…, or leave Choose… out of both.

**M5. The edit-page crumb "Edit" repeats the h1 "Edit {name}".**
- `crumbs.rs` `edit_tail` adds "Edit".
- `edit_page.rs:19` makes the h1 "Edit {subject}".
- Fix: end the crumb at the entity and let the h1 say Edit.

**M6. Empty-state copy breaks the documented rule "No X yet."**
- `design/04` section 3 fixes the wording as "No X yet." for lists that fill up.
- Agenda and Matrix use "No important tasks waiting.", "No urgent tasks waiting.", "Nothing in the backlog." and "Everything is placed." (`agenda_cards.rs:94-104`, `matrix.rs:194`).
- The report has "No sessions in this period.".
- Fix: either list these exceptions in the doc, or normalise them.

**M7. Overload on the Agenda page.**
- Counted from source, not seen rendered, the page shows:
  - The site bar and a 4-column footer. The footer repeats Settings, Boards and Organizations that the bar and the cards already offer; I saw it on Home, Boards and Settings.
  - Crumbs, h1, Edit and tabs.
  - A date label that is also a picker, with Previous, Today and Next.
  - 24 hour rows.
  - A side column with Overdue, "Important, not scheduled", "Urgent, not scheduled" and Backlog, each collapsible, plus a fold button.
- Moving a task can be done three ways: drag, a hover-only card link ("Schedule"/"Move") and the pick bar.
- Fix:
  - Make the side lists one list with filters, or fold the "Urgent" and "Important" lists together.
  - Reduce the footer to the legal and version line. Navigation is already in the bar.

**M8. The card's action link is hidden until hover on desktop.**
- `ui/task_card.css:22` sets `opacity:0` on `.tcard-act` for fine pointers over 901px.
- This keeps cards calm, but it is the only visible entry to the pick mode without JavaScript. A new user may never find it.
- Fix: show it at low emphasis (for example muted at rest) or add a first-use hint.

### Low

**L1. The same action is named differently on the two board tabs.**
- Agenda: "Skip to unscheduled tasks", list label "Unscheduled", card action "Schedule" (`agenda.rs:260`, `agenda_cards.rs:26,44`).
- Matrix: "Skip to the backlog", list label "Backlog", card action "Place" (`matrix.rs:172,194`).
- The Agenda also has a list called "Backlog" that is only one part of what is unscheduled.
- Fix: choose one noun. The two meanings of "Backlog" are the confusing part.

**L2. Header sizes are ad hoc.**
- The h1 uses a clamp up to 3rem.
- The modal and report h2 are 1.375rem, the day title is 1.25rem and the group card title is 1.0625rem.
- Section headings are small caps `.label` in some places (Projects, Sessions) and large h2 in others (report projects).
- This is acknowledged in `design/04` ("not built: type scale"), so the risk is drift.
- Fix: define 3 or 4 type tokens now. This is a small change.

**L3. Three different "segmented" controls.**
- Tabs have a sliding underline.
- The main nav and the report presets are sliding pills (`ui/frame.css`, `org.css:7-16`).
- The day nav is a row of bordered buttons.
- Fix: keep the underline for page sections. Make the report presets reuse the nav pill, or document the reason.

**L4. Status wording drifts from the doc.**
- `design/04` says "queue / wip / done".
- The UI says "queued / work in progress / done" (`models/task.rs:41-43`, seen in the Settings select).
- Fix: update the doc.

**L5. The error pages are a separate mini-site.**
- Verified by running: the 404 page has the bar with a brand link but no nav and no footer.
- A path that no route matches, such as `/nope`, returned the bare text "not found" with no HTML at all.
- Fix: route the bare 404 through the same document.

**L6. The folder picker form is not like other forms.**
- It lacks `required_note` and `form_actions` (`folders.rs:168-175`).
- It has a "Project name" field that is not marked required, although Name is required in the modal.
- Fix: use `form`, `field` and `form_actions` with the label "Use this folder".

**L7. Dead and duplicate surface.**
- The unused `add_button` (see M1).
- Both the bar nav and the footer list "Settings".
- The nav has "Board" (singular) linking to `/boards`.

## Confidence

| Item | Confidence | Basis |
|---|---|---|
| Empty states, footer, 404 behaviour, Settings text (H3 partly, L5) | High | Verified by running with an empty database |
| H1, H2, M1, M2, M4, M5, M6, L1, L3, L4, L6 | High | Direct source reading with line citations |
| H3 (kicker rule) | High for the code; whether users notice it is a judgement | Source |
| M3, M7, M8, L2 | Medium | Counts are from source. How dense it looks on screen is a judgement. |

## What I could not assess

- Real visual density and spacing, because I saw no populated pages. Seeding data needs the interactive editor, so I did not attempt it.
- Dark theme, narrow-screen and touch behaviour.
- Whether the sliding-line and pill animations feel consistent, since I did not render any.
- Screen-reader flow, including the modal focus trap and the live regions.
- Whether the hover-hidden card action is discoverable in practice.
