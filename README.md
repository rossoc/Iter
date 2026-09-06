# Iter

A CLI for tracking time across **projects** and **tasks**, with optional
tmux + GitHub integration.

- A **project** is a base directory of work (optionally a git/GitHub repo).
- A **task** is a unit of work under a project, with a status
  (`queue` / `wip` / `done`) and, optionally, a linked GitHub issue.
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
# creating projects
iter init                           # cwd becomes base_path; opens in nvim to name it
iter new some/path                  # creates the folder; base_path=that path, opened in nvim
iter clone myproj-template new-app  # copies myproj-template's fields + files (not .git/tasks/sessions)
iter clone git@github.com:me/repo   # or a git/GitHub URL (or local repo path) if no such project exists

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

`project`/`task info` and `weekday` print YAML, so you can pipe them
wherever's convenient.

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

Attaching to that tmux session starts a session for the task; detaching (or
the client going away for any reason -- including the terminal being
killed outright, not just a clean detach) ends it. `iter session elapse`
reports time on that open session. If a project has `tmux: false`, no tmux
session is created at all -- use `iter session start`/`iter session stop`
to track work on it manually instead.

`iter task done` closes any open session, kills the tmux session (if any),
removes the worktree (if any), deletes its branch (if any), and deletes the
session-config -- the task's past sessions aren't touched, since they're
keyed by task, not session-config.

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
