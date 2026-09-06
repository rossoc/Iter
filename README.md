# Iter

A CLI for tracking time across **projects** and **tasks**, with optional
tmux + GitHub integration.

- A **project** is a base directory of work (optionally a git/GitHub repo).
- A **task** is a unit of work under a project, with a status
  (`queue` / `wip` / `done`) and, optionally, a linked GitHub issue.
- Starting a **session** on a task spins up a tmux session (named
  `<project>/<task>`) and, if the project is git-backed, a dedicated git
  worktree + branch for it -- so working two tasks of the same project at
  once never means one stomps on the other's checkout.
- Every stretch of work is a **record**: a start/end pair (plus an optional
  note) against the *task*. Records are what get reported on, not
  sessions -- so if you work two tasks of the same project in parallel,
  `project info` reports the union of their time, not double the time.

## Usage

```sh
# projects
iter project new                    # opens a blank project in nvim; save & quit to create it
iter project edit myproj            # same, pre-filled with the existing project
iter project delete myproj
iter project info myproj [--date d] # name, description, time spent that day (union of all its tasks)
iter project list

# tasks
iter task new --project myproj [--issue 42]   # opens a blank (or issue-prefilled) task in nvim
iter task edit myproj/mytask
iter task delete myproj/mytask
iter task info myproj/mytask [--date d]
iter task list [--project myproj] [--status wip]
iter task done myproj/mytask        # closes any open record, tears down its session, marks done
iter task weekday myproj/mytask     # average hours per weekday

# sessions (tmux + git worktree/branch)
iter session new myproj/mytask [-b custom-branch] [--no-branch]

# manual record tracking (for tmux-disabled projects, or outside a session)
iter task start myproj/mytask
iter task stop myproj/mytask -m "note about what got done"

# GitHub
iter comment myproj/mytask "progress note"   # posts to the task's linked issue via `gh`

# tmux status
iter t                              # prints <project>/<task> for the tmux session you're in
```

`project`/`task info` and `weekday` print YAML, so you can pipe them
wherever's convenient.

## How a session works

`iter session new <project>/<task>`:
1. If the project has `github: true` and `auto_branch: true` (and
   `--no-branch` wasn't passed), creates a new git worktree at
   `<base_path>/.iter-worktrees/<task-slug>` on a new branch (default
   `feat/<task-slug>`, override with `-b/--branch`).
2. If the project has `tmux: true`, creates a detached tmux session named
   `<project>/<task>`, `cd`'d into that worktree (or the project's
   `base_path` if there's no worktree), and installs three global tmux
   hooks (idempotent, installed once): `client-attached`, `client-detached`,
   `session-closed`.

Attaching to that tmux session starts a record for the task; detaching (or
the session/client going away for any reason -- including the terminal
being killed outright, not just a clean detach) ends it. If a project has
`tmux: false`, no tmux session is created at all -- use `iter task
start`/`iter task stop` to track work on it manually instead.

`iter task done` closes any open record, kills the tmux session (if any),
removes the worktree (if any), and deletes the session -- the task's past
records aren't touched, since they're keyed by task, not session.

## Storage

Everything lives in one SQLite file, `iter.db`, at a hardcoded path (see
`DB_FILE` in `src/db.rs`) alongside where the previous CSV-based version
kept its log.

## Requirements

- `nvim` -- used to fill in a new/edited project or task as YAML. Opening
  it without saving (or saving something that doesn't parse) aborts the
  create/edit.
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
