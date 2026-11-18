| Page | URL | Light | Dark | Rules (light / dark) |
|---|---|---|---|---|
| home | `/` | 8 | 8 | target-size(serious,8) |
| settings | `/settings` | 8 | 8 | target-size(serious,8) |
| org-info | `/org/1` | 8 | 8 | target-size(serious,8) |
| org-projects | `/org/1?tab=projects` | 8 | 8 | target-size(serious,8) |
| org-projects-new | `/org/1?tab=projects&new=project` | 0 | 0 | - |
| org-tasks | `/org/1?tab=tasks` | 8 | 8 | target-size(serious,8) |
| org-tasks-new | `/org/1?tab=tasks&new=task` | 5 | 0 | color-contrast(serious,5) |
| org-notes | `/org/1?tab=notes` | 8 | 8 | target-size(serious,8) |
| org-report | `/org/1?tab=report&from=2026-10-27&to=2026-11-02` | 9 | 9 | aria-prohibited-attr(serious,1) target-size(serious,8) |
| org-report-bad | `/org/1?tab=report&from=nonsense&to=2026-11-02` | 9 | 9 | aria-prohibited-attr(serious,1) target-size(serious,8) |
| org-share | `/org/1?tab=report&from=2026-10-27&to=2026-11-02&share=1` | 1 | 0 | color-contrast(serious,1) |
| org-edit | `/org/1/edit` | 8 | 8 | target-size(serious,8) |
| org-note | `/org/1/notes/2026-11-02` | 8 | 8 | target-size(serious,8) |
| org-note-edit | `/org/1/notes/2026-11-02?tab=edit` | 8 | 8 | target-size(serious,8) |
| project-info | `/project/1` | 8 | 8 | target-size(serious,8) |
| project-loose | `/project/5` | 8 | 8 | target-size(serious,8) |
| project-tasks | `/project/1?tab=tasks` | 8 | 8 | target-size(serious,8) |
| project-tasks-new | `/project/1?tab=tasks&new=task` | 6 | 0 | color-contrast(serious,6) |
| project-notes | `/project/1?tab=notes` | 8 | 8 | target-size(serious,8) |
| project-edit | `/project/1/edit` | 8 | 8 | target-size(serious,8) |
| project-note | `/project/1/notes/2026-11-02` | 8 | 8 | target-size(serious,8) |
| task-info | `/task/1` | 8 | 8 | target-size(serious,8) |
| task-sessions | `/task/1?tab=sessions` | 8 | 8 | target-size(serious,8) |
| task-sessions-new | `/task/1?tab=sessions&new=session` | 2 | 0 | color-contrast(serious,2) |
| task-notes | `/task/1?tab=notes` | 8 | 8 | target-size(serious,8) |
| task-notes-new | `/task/1?tab=notes&new=note` | 2 | 0 | color-contrast(serious,2) |
| task-agent | `/task/1?tab=agent` | 8 | 8 | target-size(serious,8) |
| task-agent-new | `/task/1?tab=agent&new=agent` | 1 | 0 | color-contrast(serious,1) |
| task-wiki | `/task/1?tab=wiki` | 8 | 8 | target-size(serious,8) |
| task-edit | `/task/1/edit` | 8 | 8 | target-size(serious,8) |
| agents | `/agents` | 8 | 8 | target-size(serious,8) |
| boards | `/boards` | 8 | 8 | target-size(serious,8) |
| board-agenda | `/board/2?date=2026-11-02` | 8 | 8 | target-size(serious,8) |
| board-agenda-folded | `/board/2?date=2026-11-02&side=off` | 8 | 8 | target-size(serious,8) |
| board-agenda-pick | `/board/2?date=2026-11-02&pick=2` | 8 | 8 | target-size(serious,8) |
| board-agenda-moved | `/board/2?date=2026-11-02&moved=2` | 8 | 8 | target-size(serious,8) |
| board-matrix | `/board/2/matrix` | 8 | 8 | target-size(serious,8) |
| board-matrix-pick | `/board/2/matrix?pick=24` | 8 | 8 | target-size(serious,8) |
| board-matrix-moved | `/board/2/matrix?moved=24` | 8 | 8 | target-size(serious,8) |
| board-info | `/board/1/info` | 8 | 8 | target-size(serious,8) |
| board-edit | `/board/1/edit` | 8 | 8 | target-size(serious,8) |
| instructions | `/instructions` | 8 | 8 | target-size(serious,8) |
| instructions-new | `/instructions?new=instruction` | 0 | 0 | - |
| instruction | `/instruction/develop` | 9 | 9 | heading-order(moderate,1) target-size(serious,8) |
| error-400 | `/org/x` | 0 | 0 | - |
| error-404 | `/org/99999` | 0 | 0 | - |
| error-404-unmatched | `/nope` | 0 | 0 | - |
| home+palette | `/` | 51 | 51 | color-contrast(serious,1) nested-interactive(serious,50) |
| org-tasks+filter-tip | `/org/1?tab=tasks` | 8 | 8 | target-size(serious,8) |

Total violation nodes: light 367, dark 350.

| Rule | Impact | Pages | Nodes | Sample target |
|---|---|---|---|---|
| target-size | serious | 74 | 592 | `.foot-sub > li:nth-child(1) > a[href$="project/1"]` |
| color-contrast | serious | 8 | 19 | `.required-note` |
| aria-prohibited-attr | serious | 4 | 4 | `span[aria-label="Next period"]` |
| heading-order | moderate | 2 | 2 | `h4:nth-child(2)` |
| nested-interactive | serious | 2 | 100 | `#palette-opt-0` |
