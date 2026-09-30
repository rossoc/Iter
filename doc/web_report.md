# `iter serve` redesign: report

Status: final (batches A, B and C done). Nothing is committed: the work is uncommitted or untracked in the worktree `html-refactor`.

## What was asked
Redesign every page of `doc/web.md` in its order, one subagent per page. Each agent writes a plan in `doc/`, implements it, and checks it with `playwright-cli`. The result should be fast and accessible. After each page, five reviewers (accessibility, algorithmic cost, composability, DRY, design consistency) check it, and a fix agent applies their findings.

## What was done
Home and Org (info, tasks, report) were already redesigned before this session. In this session:

| Step | Result |
|---|---|
| 0. Shared groundwork | `src/web/ui/` component library (frame, tabs, status, task table, panels, forms, breadcrumb, page header…), one `/v2.css` bundle. Plan: `web_components.md` |
| 3. Org edit | `web_org_edit.md` |
| 4. Project, New task | `web_project.md` |
| 5. Project edit | `web_project_edit.md` |
| 6. Task | `web_task.md` |
| 7. Task edit | `web_task_edit.md` |
| 8. Boards | `web_boards.md` |
| 9. Board agenda | `web_board_agenda.md` |
| 10. Eisenhower matrix | `web_board_matrix.md` |
| 11. Board info | `web_board_info.md` |
| 12. Board edit | `web_board_edit.md` |
| Cleanup | `?design=old`, the compare switch, the old shell and `style.css` removed (about 1,300 lines). Plan: `web_cleanup.md` |

Every page except the cleanup went through: design agent, five reviewers, fix agent. Each page doc has a "checked against the plan" table and a "Review" table listing every finding with fixed or skipped and the reason. The final whole-codebase review (five reviewers) has been run too.

## Notable outcomes
- **Speed:** no N+1 in `src/web`. Every route runs a fixed number of queries. Tab counts use `COUNT`, finished tasks are filtered in SQL, tags and project ids are resolved in one query, and saves run in single transactions.
- **Accessibility:** skip link, landmarks, one `h1`, page titles and error prefixes on every page. Forms have labels, fieldsets, `aria-invalid` and `aria-describedby`, and the error notice takes focus. Drag and drop has a keyboard and no-JS alternative (pick mode), with focus and a status message after each move. Forced-colors and reduced-motion cases are handled. Each page passed axe with 0 violations at 1280px and 320px in light and dark.
- **Composability and DRY:** the four edit pages share one skeleton (`edit_page`, `EditForm`). Both board pages share the card, drop-zone, pick-mode and `board.js` pieces. Org and Board edit use the same project checklist.
- **Design:** one header, tabs, button, notice, panel and empty-state pattern. Colour carries meaning only.

## Batch A (verification and performance)
Checked in a browser (1280px and 320px, light and dark, axe 0 violations): the accessibility fixes A1-A6b, and the org report against `iter org info`. Added: ETag caching for the four static files, `/org.js`, and the `block_in_place` layer. Details in `web_cleanup.md` and `web_final_review.md`.

## Final status (batch C)
All three batches are done; the last one changed visible text and CSS on purpose (`web_cleanup.md`, "Batch C"). Empty states, field labels and refusals follow one voice; empty text is `--muted`; the CSS magic numbers are tokens (`--touch`, `--main-w`, `--measure`, `--sticky-head-h`, `--pickbar-h`, `--bg` on `:root`); the design wiki says what was built.

Verified on a copy of the database: 27 URLs (every route of `doc/web.md`, tabs, a bad report date, pick mode, a missing id 404, a bad id 400) at 1280px and 320px, light and dark, 108 loads: expected status, axe 0 violations, no horizontal scroll, one `h1`, a sensible title. Forced colors (current nav, tab and preset marked) and reduced motion (transitions 0s, `flash` off) on a sample. All four edit forms saved unchanged, refused (empty name, empty base path, duplicate names, bad start, duration, issue, unknown tag; typed values kept, one field marked), a task created and refused as a duplicate, drag and drop on the agenda and the matrix, keyboard pick on both, the org report equals `iter org info` (63:36, seven project totals). `cargo fmt --check`, both clippy runs, both test runs and the release build are clean.

## Known limits and follow-ups
- The `.v2` CSS scope prefix stays. Long lists are not paginated. Touch drag and drop is unreliable in browsers; pick mode is the touch path.
- No type scale (`--t-*`) or spacing scale (`--s*`): sizes and weights are literal.
- `tasks_of` sorts in Rust; `save_task` rewrites tags on every save; a connection pool and `spawn_blocking` if the server ever serves more than one person.
- Nothing is committed: the work is uncommitted or untracked in the worktree `html-refactor`.
