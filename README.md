# Iter

A CLI for tracking time across **projects** and **tasks**, with optional
tmux + GitHub integration.

- An **organization** is a group of projects that share defaults. It has no
  base_path of its own -- it isn't a place on disk, just a roster and the
  settings (`github`, `tmux`, `auto_branch`, `branch_template`,
  `github_project`) that new
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
iter new some/path                  # base_path=that path, opened in nvim; the folder is made on save
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
iter task pull myproj               # issues -> tasks (and issue state -> task status)
iter task push myproj               # tasks -> issues (and task status -> issue state)
iter task push myproj --task mytask --body   # ...just that task, description included

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

## Tasks and GitHub issues

A task's `github_issue` is the number of the issue it tracks in its
project's repo. `iter task new --issue 42` links one by hand, one task at a
time; `iter task pull` / `iter task push` do it for a whole project at once.
Both need the project to have `github: true` and its `base_path` to really
be a repo -- like `iter comment`, they run `gh` from `base_path`, so `gh`
infers the repo from its git remote and `iter` never stores an "owner/repo"
string itself.

**Each side owns one end.** `iter task pull` can create a *task* but never
touches an issue; `iter task push` can create or close/reopen an *issue* but
never changes a task's status. So running one after the other settles,
rather than the two of them arguing over the same task. Descriptions are the
one thing that can cross either way, and only when `--body` says so.

Both take the same two narrowing flags:

- `--task <name>` -- just that one task, rather than every task in the
  project. On a pull that's the single issue it tracks, so a pull scoped
  this way can only update that task; on a push it's the one task sent up.
- `--body` -- also copy the description across, in whatever direction you
  ran. Without it, descriptions only ever fill in a task or an issue being
  *created*.

```sh
iter task pull myproj    # for each issue in the repo, open and closed alike
```

- An issue nothing tracks becomes a new task: the issue's title as its name,
  its body as its description, `branch_prefix` from the project's
  `branch_template`, and its number stored.
- An issue a task already tracks hands over its state: **closed** marks the
  task `done`, and **reopened** sends a `done` task back to `queue` -- which
  is the status `iter session new` takes, so a reopened issue can be picked
  up again. An open issue leaves a `queue`/`wip` task where it is: both
  read as "open" to GitHub, and the local one says more.
- An issue whose title matches a task that tracks *no* issue links the two
  rather than creating a second task by that name -- the same work, entered
  on both sides separately. If that same-named task already tracks a
  different issue, the issue is reported and skipped instead.
- With `--body`, a tracked task's description is overwritten with the
  issue's body when the two differ.

```sh
iter task push myproj    # for each task in the project
```

- A task tracking no issue gets one opened, titled with its name and bodied
  with its description, and the new number is stored on the task.
- A task that tracks an issue has that issue closed or reopened to match:
  `done` closes it, `queue` and `wip` both leave it (or put it back) open.
  A `done` task's brand-new issue is opened and then closed, since there's
  no way to create a closed one.
- A task naming an issue the repo hasn't got -- deleted, transferred, or a
  pull request -- is reported and skipped: opening a second issue would only
  orphan the link it already has.
- With `--body`, a tracked issue's body is overwritten with the task's
  description when the two differ.

Titles are never rewritten in either direction. The issue *number* is the
identity, so renaming a task or retitling an issue changes nothing about
which is which.

### Signing off

Whenever a push closes an issue -- a `done` task's brand-new one, or one
going open -> closed -- you are added to that issue's assignees first, so a
closed issue always says who called it done. It's `--add-assignee`, so
anyone already assigned stays, and nothing is ever unassigned, including
when an issue is reopened. Nothing is assigned on an issue that stays open.

Assigning needs push access to the repo. If it fails, the push says so and
closes the issue anyway -- keeping a task and its issue in agreement matters
more than the signature.

### Filing new issues under a GitHub Project

A project's `github_project` is the title of a GitHub Project (the board --
nothing to do with an `iter` project) that newly opened issues are filed
under. Blank, the default, files them nowhere.

```sh
iter project edit myproj
```
```yaml
---
name: myproj
base_path: /home/me/code/myproj
github: true
github_project: Roadmap
---
```

Like the other GitHub settings it can be defaulted per organization: set
`github_project` on the organization and every project created in it starts
from that (see [Organizations](#organizations) -- editing the organization
later doesn't reach back into projects already created).

The name is handed straight to `gh issue create --project` and never checked
here. `gh` resolves the title against the repo owner's boards and refuses to
create the issue if there's no such board, so a name that doesn't resolve
costs you a failed push rather than issues filed in the wrong place -- and
nothing is created in the meantime. It only applies to issues `push`
*creates*; an issue that already exists is never moved between boards.

Reaching Projects at all needs a token that can: `gh auth refresh -s project`
on a classic login, or the Projects permission granted explicitly on a
fine-grained token. Without it `gh` reports `Resource not accessible by
personal access token` and the push fails.

## Organizations

An organization holds two things: the list of projects that belong to it,
and the defaults those projects start from.

**Defaults flow downstream, once.** `--organization` on `iter init`/`new`/
`project new` seeds the blank template with the organization's `github`,
`tmux`, `auto_branch`, `branch_template` and `github_project` before nvim
opens, so you can
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

A project always ends up with a non-empty, absolute base_path: blank it in
the editor and nothing is created, and a relative one you type there is
resolved against the directory you ran the command from -- a project whose
base_path is relative would mean a different folder from everywhere `iter`
is run later.

- `iter init` -- for a folder you're already in: base_path is the current
  directory. Nothing is created on disk.
- `iter new <path>` -- for starting completely fresh: `<path>` becomes
  base_path and is created (like `mkdir -p`) when you save. Nothing is
  created before that, so quitting nvim without saving leaves no folder
  behind -- and if you change base_path while editing, it's the path you
  changed it to that gets created.
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

`github`/`tmux`/`auto_branch`/`branch_template`/`github_project` all default the same as
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

A tmux session can have more than one client on it (a second terminal, or
someone pairing), and each of them detaches separately, so only the *last*
one out stops the clock: a detach that leaves somebody else attached
changes nothing. If you drive the detach from your own `tmux.conf` hook
rather than the ones `iter` installs -- calling `iter session stop`
directly, say -- that's an explicit stop and it stops the session whoever
else is attached.
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

Everything lives in one SQLite file, `iter.db`, kept in `iter`'s config
directory next to the config file itself -- so the two travel together, and
pointing `$XDG_CONFIG_HOME` somewhere else moves both. `db_path` in the
config file overrides where the database goes, if you'd rather keep it
somewhere backed up or synced.

## Configuration

Optional, and optional key by key: with no config file at all -- or an empty
one -- every value below is what `iter` uses anyway. It lives at
`$XDG_CONFIG_HOME/iter/iter.yaml`, which on a machine that doesn't set that
variable means `~/.config/iter/iter.yaml` (`%APPDATA%\iter\iter.yaml` off
unix).

```yaml
# Where the SQLite file lives. Default: iter.db beside this file. A leading
# `~` is expanded; a relative path is resolved from wherever you run `iter`.
db_path: ~/.config/iter/iter.db

# A gap between one session's end and the next one's start shorter than this
# is treated as a pause inside one stretch of work rather than a real break
# between two -- so a quick interruption doesn't split a morning in half.
pause_gap_minutes: 17

# Opened to fill in a new/edited organization, project or task. It's handed
# one argument, the path of a temporary markdown file, and quitting it
# without saving aborts the create/edit.
editor: nvim

# What `iter project new` -- and `iter init`/`new`, and `iter clone` of a git
# URL -- puts in the buffer before you edit it. (Cloning an existing project
# copies that project's own settings instead.) `iter organization new` starts
# from this same block, since an organization's fields *are* the defaults its
# projects inherit.
project:
  github: false
  tmux: true
  auto_branch: true
  branch_template: feat/{task}
  # The GitHub Project board `iter task push` files new issues under, by
  # title. Empty files them nowhere -- worth setting here only if your
  # boards are named the same across repos; otherwise set it per project.
  github_project: ''

# What `iter task new` starts a task at: queue, wip, or done.
task:
  status: queue
```

These are starting points, not rules: they're written into the editor buffer
as ordinary front matter, so any of them can be changed there before you
save, and none of them touches anything that already exists. An organization
outranks them for its own projects -- being the narrower answer to the same
question -- and `github` is overruled by what's actually on disk, since
`iter init`/`new` won't call a directory a repo when it isn't one.

A key that isn't recognised is an error rather than something quietly
ignored, so a typo can't leave you with a setting you think you set.

## Requirements

- `nvim`, or whatever `editor` in the config file names -- used to fill in a
  new/edited organization, project or task. It opens a temporary `.md` file:
  the fields as YAML front matter at the top, the description as the
  markdown body below it -- so the editor highlights and lints it the way it
  would any other markdown file with front matter. Quitting without saving
  (or saving something that doesn't parse) aborts the create/edit.
- `tmux` -- for session support (skippable per-project via `tmux: false`).
- `git` -- for worktree/branch support (skippable per-project via
  `github: false`, or per-session via `--no-branch`).
- `gh` (GitHub CLI, already authenticated) -- for `--issue` task creation,
  `iter task pull`/`iter task push` and `iter comment`. All of them run from
  the project's `base_path`, so `gh` infers the repo from its git remote --
  `iter` never stores an "owner/repo" string itself. Filing issues under a
  `github_project` additionally needs a token with Projects access.

## Shell completion

```sh
source <(COMPLETE=zsh iter)
```

gets you tab-completion on project names (`project edit/delete/info`,
`task new --project`, `task pull`/`task push`) and `<project>/<task>` pairs
(everywhere a task is expected).

## Building

```sh
cargo build --release
```
