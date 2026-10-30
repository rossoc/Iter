# The Agent tab (`/task/{id}?tab=agent`)

The third tab of the task page (Info, Sessions, Agent). It shows the task's plan and what the task's folder holds, and lets a person move a step between todo, doing and done. Code: `src/web/task_agent.rs`, rules in `src/web/ui/steps.css`; the files and their format are `src/agent/`.

## What it shows

Two columns, like the Info tab:

- Left: **Plan** (progress bar and "3 of 7 done, 1 in progress. Checks 1 of 2", then Goal, How and the Checklist), **Notes** and **Sketches** (each file a `<details>`, the markdown rendered by `ui::prose`, a file cut at 64 KB, at most 50 files).
- Right: the open **step** (its text and detail, its state, the buttons that move it, "Close"), or, with no step open, **Prompts for an agent**: the `iter task prompt` commands and the task's folder.

States: no plan ("No plan yet."), no notes folder (the project and its organization have no `notes_path`: the tab says to set one), a `?step=` that is not in the plan (a line saying so, the prompts panel).

## Steps

A step is a link, `?tab=agent&step=s3`, so opening one needs no script; the current one has `aria-current="step"`. A step written by hand and not numbered yet has no id and is not a link (`iter task plan` numbers it). Each row says its state in text for a screen reader, as well as by its icon (the icons are the task status ones: circle, half, check).

`POST /task/{id}/plan/{step}` takes `state=todo|doing|done`, changes that step's checkbox in `plan.md` (nothing else in the file changes) and redirects (303) to the same step. A state that is not one of the three is a 400; no such task, no notes folder, no plan or no such step is a 404, and the file is left as it was.

## Not here yet

The harness picker and a button that starts an agent need the runner (`iter task solve`). Until then the panel lists the commands.

## Runs and the launch form

The Runs section lists the latest ten runs of the task (`tasks/<id>/runs/`), newest first. Each is a `<details>`: the summary says the number, the state (as text, `running`, `done`, `failed` or `lost`; colour is not the only cue), what it was asked, the harness, the model and when it started; opened it shows the branch and the end of the log (the last 16 KB, in a focusable `<pre>`).

"Hand to an agent" is a plain form (`POST /task/{id}/solve`, no script): what to do, agent, model, effort. The server checks every value (a model that starts with `-` or has a space is refused, since it becomes an argument of the harness), refuses a second run while one is `running`, starts the run detached and redirects back to the tab. The page does not refresh by itself; reload to follow a run.
