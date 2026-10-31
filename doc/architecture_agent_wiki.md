# Agent, wiki, push and report architecture

Design notes for `src/agent`, `src/wiki`, `src/push.rs`, `src/sync.rs`, `src/platform.rs` and `src/reporting`. Current behaviour only.

## Agent: task folder and plan (`src/agent/taskdir.rs`, `plan.rs`)

- A task's folder is `tasks/<id>/` inside its project's notes folder (else in iter's config directory when neither project nor organization has notes). It holds the plan, the task's own notes, the agent's sketches and `runs/`. It lives with the notes, not in the worktree, so discarding a branch loses nothing planned or learned.
- The plan is `plan.md`, a markdown file and the only store:

  ```text
  ## Goal
  What done looks like, in a few lines.

  ## How
  The approach, and what is hard about it.

  ## Checklist
  ### Phase (optional big step)
  - [x] {s1} Write the parser
  - [~] {s2} Wire it into the CLI
  - [ ] {s3} (verify) `cargo test` passes
      detail lines indented under a step belong to it
  ```

- Reading is forgiving (a person edits the file too): other `##` sections are kept and ignored, a step needs only its checkbox, `*` works for `-`, any indent. A trailing `(verify)` / `(verify: how)` marks a step that checks rather than does; only the last parenthesis counts. A `##` inside the YAML front matter is not a heading; an unclosed front matter is not one.
- Writing is careful: ticking a step edits the one character (`[ ]`, `[~]`, `[x]`) and every other byte survives, including the final newline or its absence.
- Step ids (`{s3}`) are given once and never reused while the step exists; a missing or duplicate id gets the next free `s<n>`. This lets a link, button or agent name a step however the plan is reordered or reworded.
- Saving a plan numbers every step; an empty plan is refused; a missing Goal or Checklist is saved and reported (a task may need another shape). Replacing a plan keeps the previous one (`previous_plan`), one level of undo.
- Plan writes are atomic (unique temp file beside the target, then rename) and read-modify-write cycles hold the plan's lock, so an agent ticking a step and a click on the page take turns.

## Agent: prompts and instructions (`prompt.rs`, `instruction.rs`, `task_prompt.rs`)

- A prompt is an instruction (markdown, opencode agent format: front matter `description`, `model`, `mode`, `temperature`, `permission`, plus iter's `kind`) with `{{placeholders}}` filled from the task: name as `<project>/<task>` (shell-escaped), worktree, plan, wiki onboarding. An unknown placeholder is an error, never a literal in the agent's prompt.
- Kinds: `plan`, `develop`, `test`, `check`, `fix`. `plan` and `check` change no code; the others work in the task's worktree. Old run records may say `execute` / `investigate`; they still read.
- Lookup is nearest first: `tasks/<id>/instructions/<kind>.md`, the project's notes, the organization's notes, iter's config folder, then the built-in template. An explicit template beats an override.
- The wiki reaches the agent side in one place, `task_prompt`'s onboarding; it never fails the prompt (no wiki, nothing found or any error gives no text).

## Agent: runs (`run.rs`, `solve.rs`, `exec.rs`)

- A run is one headless harness process working on a task in its worktree. Files in the task's folder: `runs/<n>.yaml` (record), `runs/<n>.prompt.md` (the prompt given), `runs/<n>.log` (harness output). `iter task runs` and the Agent tab read the same files.
- Numbers are claimed by creating the record file with a call that fails if it exists, so concurrent starts get distinct numbers. One run at a time per task.
- States: `queued` (written down, editable, not started) -> `running` -> `done` | `failed`; `cancelled` from queued or running. `lost` is computed when read: a run saying `running` whose process is gone (killed, machine restarted), or one that never got a process after a minute.
- Lifecycle: `create` (record + prompt; `{{worktree}}` stays until begin) -> `amend` (only while queued) -> `begin` (make the workspace if the kind changes code, before the prompt so it names the right directory; mark the task `wip` unless done; start the harness, waited for or detached) -> `worker` (the detached process: runs the harness to the end, numbers steps the agent left without ids, records the outcome). Anything that goes wrong after begin starts is recorded as `failed`, never left `running`.
- `cancel` writes the record first (the worker leaves a cancelled run as it is when it wakes), then signals the process group. A detached child is started in its own process group so a signal to iter's does not reach it; a thread waits on it to avoid a zombie in a long-lived server.
- A harness is a closed set (`Harness`) dispatching to an `AgentBackend` that turns an `Invocation` (prompt, model, effort, extra directories, allowed tools, extra arguments) into a program and arguments. Headless means nobody to ask, so edits are allowed (`--permission-mode acceptEdits`) plus the commands the prompts name.
- The first run of a task may trigger a wiki ingest if the project's `ingest` setting says so (see below).

## Wiki: graph model (`src/wiki`)

- The markdown is the only source of truth. Every call rebuilds an in-memory `oxigraph` store (`graph::Wiki::build`), so nothing can disagree and there is no index file. Facts are loaded in one go: each store insert is a transaction, so inserting thousands one by one costs seconds.
- Levels are directories with a meaning: an organization's, a project's or one task's folder. The caller (which has the database) chooses the levels in scope; `scope.rs` turns a task/project/organization into them, innermost first. A task sees its own folder, its project's and its organization's wiki; an organization may go `down` into its projects' wikis, never into a task's folder (private working memory).
- The class of a page comes from its folder as the ontology says (`ontology.ttl`, extended by a project's or organization's `wiki/ontology.ttl`); front matter never repeats it. Front matter is strict YAML (`frontmatter.rs`): description, tags, confidence, update time and typed relations (the keys the ontology marks `iter:authorable`); other keys land in `unknown` for `check`. `[[links]]` in a body are `iter:ref` relations. Organizations, projects and tasks are nodes too, each `iter:in` the level whose folder holds it. Raw notes (`iter:Note`) are listed but never read.
- Ids are paths: `label:path` for another level, bare when the path names the node from the asking level (nearest level wins), `wiki/` optional. An id always resolves back to its node when asked from the same level.
- The ontology is OWL but oxigraph reasons about none of it, so `graph::entail` applies the OWL 2 RL subset the ontology uses (subclass, subproperty, inverse, symmetric, transitive) before any query. Domains and ranges are constraints for `check`, not rules. Each fact is processed once as it appears; derived facts are queued and a transitive property joins with facts seen so far. A rule that would make a literal a subject is skipped. SPARQL is read-only: the markdown is where things change.
- Folders named by an ontology are checked to stay inside the level (no `..`, no root) since `mv` would otherwise rewrite files outside the wiki. File listing skips dotfiles and does not follow links.
- `check` is the lint that needs no LLM (severities: error, warning, info). Sketches are the agent's working memory and only have to parse. A note needs ingesting when no page has it as a source or it changed after the pages made from it were updated (file mtime against `updated`; dates count as end of day; no hash, no ledger).
- `mv` renames a file inside its level and rewrites every reference (front matter values, body links) keeping each spelling; a page must stay in a folder that gives it a class; a failed move puts back what it changed. Raw notes are never rewritten.
- `retrieve` reads cheaply in four steps of rising cost: `find` (rank by path, title, tags, description), `grep`, `neighbors` (follow relations, backlinks included, `nb` pages first), `show` (a page at a chosen depth). `find` and `grep` stop at a token budget (about four bytes per token).

## Wiki: ingest and review (`src/wiki/ingest*.rs`, `ingest_job/`)

- Ingest turns notes into pages: `wiki::ingest` finds pending notes and words the prompts (ingest notes, build a concept from ideas, with `schema::render` rules last so nothing in them reads as a placeholder); an agent run does the writing. The `ingest` setting (`manual`, `on_task_start`, `on_task_done`) lives in `wiki/settings.yaml` of the project, inherited from its organization; the innermost level that sets one wins.
- One job writes a project's wiki at a time: a lock file made with a call that fails if it exists; a lock left by a dead process is replaced. A second ingest or a review while an agent is writing is refused (`WikiBusy`, `WikiWriting`).
- The agent's changes stay uncommitted in the notes' git repository (so it must be one, else an ingest is refused). `pending` lists the uncommitted `.md` pages under `wiki/` (settings and ontology extension are the user's and not listed; nothing is written, not even git's index). Accepting is a commit of those pages and no other file; reverting takes them out of the index, restores the last commit and removes new pages.
- Detached ingest is `iter wiki ingest` started as its own process; the harness log is under `.iter/` in the notes folder, which git ignores. Arguments use `--flag=value` so a name starting with `-` is a value.
- `task done` bridges the task's own notes into the project's note of the day under `## Task: <name>` (one `###` per file, once), then may trigger an ingest; neither stops the task being done.

## Push and sync (`src/sync.rs`, `commands/sync.rs`, `push.rs`, `platform.rs`)

- GitHub sync: `sync.rs` plans each issue/task as plain data; `commands/sync.rs` carries plans out. Deliberately asymmetric: pull can create a task but never touches an issue; push can create or close/reopen an issue but never changes a task's status, so statuses cannot ping-pong. The issue number is the identity (names and titles are editable), the title is consulted only for an issue no task has claimed. Descriptions are the exception: `--body` copies one over the other in the direction run, otherwise they are left alone (a new task or issue always takes the other side's text). A reopened issue puts its task back to `queue`.
- Push to a time platform has three steps shared by the CLI and the web page: `plan` (the worklog and GitHub work that would happen; nothing leaves the machine), `check` (ledger and platform conflicts), `execute` (platform first; only if it accepts a day, the GitHub comment and close for tasks completed on that day; then the ledger row).
- The ledger (`models::Push`) stores a fingerprint of what was sent: the same worklog gives the same one, a corrected one (another figure, a session closed since) another, so re-pushing an unchanged worklog is recognised. An open session marks the preview and changes the fingerprint.
- Days the platform already holds are conflicts and are not sent (a day has one entry); the rest go through. A request refused as a whole is an error with GitHub untouched and no ledger row. A day pushed by an earlier incomplete run of the same worklog is ours: finishing it comments the issue and does not push the day twice or report a conflict. Each item's outcome is reported, never swallowed.
- Projects/activities may be given as names or ids; resolution to ids uses the platform's lists. No lists means the text is sent as is; a list that fails to load is an error (a name where an id is expected files hours under the wrong project).
- Time platform JSON interface (`TimePlatform`), paths relative to the organization's `platform_url`, every call with `Authorization: Bearer <key>`; the key never appears in an error (everything coming back is redacted):
  - `GET /projects` -> `{"projects": [{"id", "name"}]}`.
  - `GET /projects/{project-id}/activities` -> `{"activities": [{"id", "name"}]}`.
  - `GET /time-entries?from=yyyy-mm-dd&to=yyyy-mm-dd[&page=n]` -> `{"entries": [{"date", "time", "note", "project", "activity", ...}], "has_more"}`; both days included, `time` in minutes, `project`/`activity` ids. Reading stops at a page cap; exceeding it is an error, not "no more entries".
  - `POST /time-entries` with `{"entries": [{"project", "activity", "date", "time", "note"}]}`, one entry per day. An already-booked day is refused alone: the response is 2xx with `{"results": [{"day", "status": "ok" | "conflict", "message"}]}`. No results means every entry was accepted; an unrecognisable 2xx body (a proxy's or captive portal's page) is not an acceptance.

## Reports (`src/reporting`)

- `info` reports are built from sessions: `project_reports` returns the per-project/task reports and the union of sessions. Time is the merge of chronologically adjacent sessions whose gap is under a threshold (`merged_total_minutes`), so tasks worked in parallel are not double-counted at project or organization level (the union is taken across tasks, not summed).
- Header scalars (name, period, time, the row's settings) are the YAML front matter of a text report and are flattened into the YAML format, so both carry the same fields. The row's settings come from `Serialize` in declaration order, minus `id` and `name`. The period serializes as `date:` for one day, `from:`/`to:` for an interval; an open start leaves `from:` out. A one-day report drops the `Date` column.
- Formats: text (markdown with fenced front matter, tables padded to be read in a terminal), YAML, and `worklog`.
- Worklog: one entry per day (the shape the platform receives), a day's hours are the merged total of that day's sessions, one `- ` line per task: `Completed <task>` on the last day a done task was worked on in the period, `Worked on <task>` otherwise (tasks have no completion date, so `done` means finished at some point and worked on in the period). Entries add up to what `info` reports. A still-open session is measured against now and marks the worklog so a later push may differ.
