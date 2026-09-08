# Iter

A CLI for tracking time across **projects** and **tasks**, with optional
tmux + GitHub integration.

- An **organization** is a group of projects that share defaults. It has no
  base_path of its own -- it isn't a place on disk, just a roster and the
  settings (`github`, `tmux`, `auto_branch`, `branch_template`) that new
  projects in it start from. Belonging to one is optional: a project without
  an organization behaves exactly as it always has.
- A **project** is a base directory of work (optionally a git/GitHub repo).
- A **task** is a unit of work under a project, with a status
  (`queue` / `wip` / `done`), a `branch_prefix` for the branch its session
  gets, and, optionally, a linked GitHub issue.
- `iter session new` on a task creates a **session-config**: a tmux session
  (named `<project>/<task>`) and, if the project is git-backed, a dedicated
  git worktree + branch for it -- so working two tasks of the same project
  at once never means one stomps on the other's checkout.
- Every stretch of work is a **session**: a start/end pair (plus an optional
  note) against the *task*, started/stopped via `iter session
  start`/`iter session stop` (manually) or by attaching/detaching from the
  tmux session (automatically). Sessions are what get reported on, not
  session-configs -- so if you work two tasks of the same project in
  parallel, `project info` reports the union of their time, not double the
  time.

## Usage

```sh
# creating projects  (--organization/-o is optional on all of these)
iter init                           # cwd becomes base_path; opens in nvim to name it
iter new some/path                  # creates the folder; base_path=that path, opened in nvim
iter clone myproj-template new-app  # copies myproj-template's fields + files (not .git/tasks/sessions)
iter clone git@github.com:me/repo   # or a git/GitHub URL (or local repo path) if no such project exists
iter init -o acme                   # ...and start from organization `acme`'s defaults

# organizations  (alias: `iter org`)
iter organization new               # opens a blank organization in nvim; save & quit to create it
iter organization edit acme
iter organization delete acme       # deletes only the grouping; its projects are kept
iter organization info acme         # what the whole organization spent that day, per project and per task
iter organization list

# projects
iter project new [-o acme]          # opens a blank project in nvim; save & quit to create it
iter project edit myproj [-o acme]  # same, pre-filled; -o moves it into that organization
iter project delete myproj
iter project info myproj [--date d] # name, description, time spent that day (union of all its tasks)
iter project list
iter project                        # same as `project list`

# every info command takes --format/-f and the same period flags
iter task info myproj/mytask -f yaml    # YAML instead of the default markdown report
iter org info acme --from 2026-09-01    # an interval (up to today) instead of a single day

# tasks
iter task new --project myproj [--issue 42]   # opens a blank (or issue-prefilled) task in nvim
iter task edit myproj/mytask
iter task delete myproj/mytask
iter task info myproj/mytask [--date d]
iter task list [--project myproj] [--status wip]
iter task                           # every unfinished task (queue + wip), across all projects
iter task done myproj/mytask        # closes any open session, tears down its session-config, marks done
iter task weekday myproj/mytask     # average hours per weekday

# sessions (tmux + git worktree/branch, and time tracking)
iter session new myproj/mytask [-b custom-branch] [--no-branch]
iter session elapse                  # time spent so far in the tmux session you're in

# manual time tracking (for tmux-disabled projects, or outside a session)
iter session start myproj/mytask
iter session stop myproj/mytask -m "note about what got done"

# GitHub
iter comment myproj/mytask -m "progress note"   # posts to the task's linked issue via `gh`

# tmux status
iter t                              # prints <project>/<task> for the tmux session you're in
```

## Reports

`organization info`, `project info` and `task info` are the same report at
three depths -- organization to project to task to session -- and all three
take the same two knobs.

**Which days.** Either `--date <YYYY-MM-DD>` for a single day (the default,
today), or `--from`/`--to` for an interval; one formulation or the other,
never both. Each end of the interval is optional: `--from` alone runs up to
today, `--to` alone reaches back over every session there is.

**How it's printed.** `--format`/`-f` is `text` (the default) or `yaml`.

`text` is markdown: a YAML front matter block of the settings and totals,
then the description and the breakdown as the body -- the same shape the
nvim editor opens a project or task in, so a report reads like the thing it
reports on (and, saved to a `.md` file, is highlighted and linted as one).

```sh
$ iter organization info acme --date 2026-09-07
---
name: acme
date: 2026-09-07
total_hours: 4.5
total_hhmm: 04:25
settings:
  github: false
  tmux: true
  auto_branch: true
  branch_template: feat/{task}
---

# acme

2026-09-07 -- 04:25 (4.5 h)

Everything 4BC works on.

| Project | Time  |
|---------|-------|
| Iter    | 04:25 |

## Iter

04:25 (4.5 h)

The time tracker itself.

| Task  | Status           | Time  |
|-------|------------------|-------|
| Doc   | work in progress | 03:45 |
| fixes | queued           | 00:40 |

### Doc -- work in progress

03:45 (4.0 h)

Write the README section on formats.

- wrote the format docs
- shipped it

| Start | End   | Duration | Message               |
|-------|-------|----------|-----------------------|
| 09:00 | 11:15 | 02:15    | wrote the format docs |
| 13:00 | 14:30 | 01:30    | shipped it            |
```

`project info` is that same document one level down (`# project`,
`## task`), and `task info` one level down again -- just the task, its
description and its session table.

An open session shows no `End` and is marked `(ongoing)`; its duration runs
to now. Over an interval every session table grows a `Date` column.

`-f yaml` prints the identical data as plain YAML -- the front matter fields
flat at the top, then the same tree nested under `projects:` / `tasks:` /
`sessions:` -- for piping somewhere else. (`weekday` prints YAML too, and
has no `--format` of its own.)

A project or task with no session in the period is left out of the report
entirely; `organization list` / `project list` / `task list` are the place
for a plain roster of what exists.

**Totals are unions, not sums.** A project's time is the union of its tasks'
sessions and an organization's is the union of its projects', so working two
things in parallel -- or hopping between them -- is counted once. That's why
a parent's total can be less than its children's totals added up.

## Organizations

An organization holds two things: the list of projects that belong to it,
and the defaults those projects start from.

**Defaults flow downstream, once.** `--organization` on `iter init`/`new`/
`project new` seeds the blank template with the organization's `github`,
`tmux`, `auto_branch` and `branch_template` before nvim opens, so you can
still change any of them before saving. They're a starting point, not a
constraint: editing the organization later doesn't reach back into projects
already created. Two exceptions, both because a fact beats a default:
`init`/`new` set `github` from whether the directory really is a git repo,
and `clone`ing a repo forces it true.

`iter clone <existing-project>` is the odd one out: cloning a project means
keeping *that project's* settings, so `-o` there only sets which
organization the copy belongs to.

**Membership is optional and reversible.** Projects created before
organizations existed simply have none, and keep working unchanged; `iter
project edit myproj -o acme` moves one in. Deleting an organization deletes
only the grouping -- every project, task and session survives, and the
projects go back to belonging to none.

## How a project gets created

Three ways to get from nothing to a registered project, all ending the same
way: base_path is filled in, then nvim opens for you to name it (and set
anything else) before it's saved to the database.

- `iter init` -- for a folder you're already in: base_path is the current
  directory. Nothing is created on disk.
- `iter new <path>` -- for starting completely fresh: `<path>` is created
  (like `mkdir -p`) and becomes base_path. If you quit nvim without saving,
  the folder is removed again (only if it's still empty).
- `iter clone <source>` -- for templating off something that already
  exists:
  - If `<source>` is the name of an existing project, its fields (including
    its name -- you'll need to change that before saving) are pre-filled
    into the editor, and on save its files are copied into the new
    base_path you typed in, `.git` excluded. The source project's tasks and
    sessions are never touched.
  - Otherwise `<source>` is treated as a git remote URL or a local repo
    path, and cloned into the base_path you typed in via a plain `git
    clone <source> <base_path>` -- exactly like running `git clone`
    yourself.

`github`/`tmux`/`auto_branch`/`branch_template` all default the same as
`project new` for `init`/`new` (`github` pre-set to `true` if the resulting
directory already looks like a git repo), and are carried over as-is from
the source project for a local `clone`.

## Branch names

A project's `branch_template` (default `feat/{task}`) is only a *default*:
its fixed part is copied into each new task's own `branch_prefix` field
when the task is created, so it's sitting right there in the editor to
change per task. `iter session new` branches from that prefix plus the
task's slugified name -- `hotfix/` + `Fix Login Bug` gives
`hotfix/fix-login-bug`. A task whose `branch_prefix` is empty (including
every task created before the field existed) falls back to the project's
`branch_template`.

## How a session works

`iter session new <project>/<task>`:
1. If the project has `github: true` and `auto_branch: true` (and
   `--no-branch` wasn't passed), creates a new git worktree at
   `<base_path>/.iter-worktrees/<task-slug>` on a new branch named
   `<the task's branch_prefix><task-slug>` (override the whole name with
   `-b/--branch`).
2. If the project has `tmux: true`, creates a tmux session named
   `<project>/<task>`, `cd`'d into that worktree (or the project's
   `base_path` if there's no worktree), installs three global tmux hooks
   (idempotent, installed once) -- `client-session-changed`,
   `client-detached`, `session-closed` -- and attaches you to it (switching
   the current client, if you're already inside tmux).

Entering that tmux session starts a session for the task, and leaving it
ends one. `client-session-changed` covers both halves of the common case:
it fires on a plain attach and on `switch-client`, so moving from one task
straight to another stops the first and starts the second. `client-detached`
catches the client going away for any reason -- including the terminal
being killed outright, not just a clean detach -- and `session-closed` is
the backstop for the tmux session being destroyed while still attached.
`iter session elapse` reports time on the open session. If a project has
`tmux: false`, no tmux session is created at all -- use `iter session
start`/`iter session stop` to track work on it manually instead.

Each hook is told which session it fired for by a format variable, and
which variable is correct **differs per event** -- `#{session_name}` for
the client hooks, `#{hook_session_name}` for `session-closed`, where
`#{session_name}` would name a different session entirely. Getting it wrong
is silent: the hook still runs, it just names no session `iter` knows and
does nothing. `tmux::HOOKS` has the measurements, and there are tests
pinning each one.

**Detach messages.** Before detaching, anything can leave a note in the
tmux session option `@iter_detach_message`; the `client-detached` hook
takes it off again and stores it on the session it closes, the same as
`iter session stop -m`. That's what a `tmux.conf` binding like this hangs
off:

```tmux
# prompt for a message in a popup, stash it, then detach
bind D run-shell "~/bin/detach_with_message.sh '#{client_name}'"
```

`iter task done` closes any open session, kills the tmux session (if any),
removes the worktree (if any), deletes its branch (if any), and deletes the
session-config -- the task's past sessions aren't touched, since they're
keyed by task, not session-config.

## Storage

Everything lives in one SQLite file, `iter.db`, at a hardcoded path (see
`DB_FILE` in `src/db.rs`) alongside where the previous CSV-based version
kept its log.

## Requirements

- `nvim` -- used to fill in a new/edited organization, project or task. It
  opens a temporary `.md` file: the fields as YAML front matter at the top,
  the description as the markdown body below it -- so the editor highlights
  and lints it the way it would any other markdown file with front matter.
  Quitting without saving (or saving something that doesn't parse) aborts
  the create/edit.
- `tmux` -- for session support (skippable per-project via `tmux: false`).
- `git` -- for worktree/branch support (skippable per-project via
  `github: false`, or per-session via `--no-branch`).
- `gh` (GitHub CLI, already authenticated) -- for `--issue` task creation
  and `iter comment`. Both run from the project's `base_path`, so `gh`
  infers the repo from its git remote -- `iter` never stores an
  "owner/repo" string itself.

## Shell completion

```sh
source <(COMPLETE=zsh iter)
```

gets you tab-completion on project names (`project edit/delete/info`,
`task new --project`) and `<project>/<task>` pairs (everywhere a task is
expected).

## Building

```sh
cargo build --release
```
