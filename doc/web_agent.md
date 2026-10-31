# The Agents tab (`/task/{id}?tab=agent`)

The third tab of the task page (Info, Sessions, Agents, Notes, Wiki, Settings); its badge is the number of agents. It shows the agents that work on the task and the plan's steps. Code: `src/web/task_agent.rs` (the tab, the plan, the files), `src/web/task_runs.rs` (the agents), rules in `src/web/ui/steps.css`; the files and their format are `src/agent/`.

## What it shows

- Left: the plan as a contents list (like the report's): the plan's **big steps** (the `### name` headings of its checklist), each with its count and a bar of how many of its small steps are done, and the small steps under it (links, `?tab=agent&step=s3`; the current one has `aria-current`). The small steps are not listed there; a plan without headings lists its steps, each with a bar of its own. Not shown when there is no checklist; narrow, it lies above the rest.
- Right: the agents table (no heading: the tab's badge has the count), with what is open over it (a step, or an agent).
- The plan (Goal, How and the **checkpoints**: every step with a checked glyph when done, half when doing, under its big step; each a link to the step's panel here) is on the Info tab, under the description, when there is a plan. Notes are the Notes tab; sketches are at the end of the Wiki tab.

The plan is one per task: `plan` is the first instruction and the only one the New agent form offers until there is a plan. The plan's prompt asks for big steps named after the instructions that will do them (`develop`, `test`, `check`, `fix`, or the task's own) with as few small steps as each needs; from then on the form offers every instruction that applies (the task's `instructions/` folder, the project's, the organization's, iter's, the built-in ones).

## Agents

The table has the tasks table's look: a first row "New agent…", then one row per run (`tasks/<id>/runs/`), the latest first: the instruction (and, on the row under the pointer, model and effort, to its right), the state (text, and a breathing dot while it works) and the buttons that fit it: a waiting run has **Run now** (a play icon) and **Remove from the queue**; a working one has **Stop**. States: `queued`, `running`, `done`, `failed`, `cancelled`, `lost`. The start time is stored but not shown.

- **New agent…** opens a pop-up (as New task does; `?new=agent&instruction=fix`): the **instruction** and the **model** are text fields with suggestions (a native datalist: click for all of them, type to filter; any text can be typed, the server refuses a model or an instruction that does not exist); choosing an instruction loads its prompt for this task by itself (`modal.js` fetches `GET /task/{id}/prompt?instruction=NAME`, JSON `{prompt, model}`, and puts them in the form) -- unless the prompt was edited since it was loaded (it differs from the text area's `data-loaded`): that text is never replaced, and the model is only set while it is untouched; without script, Enter in the field submits the small GET form, which reloads the pop-up with that instruction; agent and effort are lists (the models are `agent.models` of `iter.yaml`, else the harness's own); the **prompt** is a text area filled with the task's information, to edit. Buttons: **Run now** (play icon), **Queue** (writes the agent down, waiting, like an image that is not a running container: nothing starts until Run now) and Close. `{{worktree}}` stays in the text until the run begins, when the workspace is known. A waiting agent opens in the same pop-up (Save, Run now, Queue, remove).
- Clicking an agent opens its **page** (`/agent/{task}/{n}`, below); the Agents tab itself only lists and starts them.

## Agents page and agent pages

"Agents" in the top bar (a robot icon) lists every agent of every task, the latest first (`/agents`, `src/web/agents.rs`): the agent (`#3 Develop`, model and effort on hover), its task, its state. An agent's page (`/agent/{task}/{n}`) has the crumbs Agents / task / #n, a small box on the right with the facts (state, instruction, agent, model, effort, task, times, exit code, branch) and the buttons that fit the state (Run now and Remove for a waiting agent; **Stop** for a working one: the worker and the harness under it are told to stop, the run is `cancelled`), and the tabs **Prompt** (what it was told, rendered as markdown), **Output** (the end of the log, 16 KB, rendered as markdown; "Show raw" shows it as it is, Refresh reloads a working agent's; the page does not refresh by itself) and, only while the agent waits, **Settings** (model, effort and prompt, with Save, Run now and Queue). Once the agent has begun nothing can change, so Settings is not there and `?tab=settings` opens the default tab: Prompt for a waiting agent, Output for one that has begun.

Routes (all plain forms, no script; the origin check of `web/guard.rs` applies): `POST /task/{id}/runs` (create; `do=run|queue`), `POST /task/{id}/runs/{n}` (change a waiting one; `do=save|run|queue`), `POST /task/{id}/runs/{n}/run`, `POST /task/{id}/runs/{n}/cancel`. A refusal (a second run while one works, a project that is not a git repository, a model that is not offered) is a 400 with the message.

## Steps

`POST /task/{id}/plan/{step}` takes `state=todo|doing|done`, changes that step's checkbox in `plan.md` (nothing else in the file changes) and redirects (303) to the same step. A step written by hand and not numbered yet has no id and is not a link (`iter task plan` numbers it). A state that is not one of the three is a 400; no such task, no notes folder, no plan or no such step is a 404, and the file is left as it was.

## Sessions

The Sessions tab is a table like the tasks one (hover highlight, first row "New session…" which opens a form above the table: start, end, note; `POST /task/{id}/sessions`, a session must end after it starts).

## Instructions

An instruction is what an agent is told for a kind of work: a markdown file in the format of an opencode agent (front matter `description`, `model`, `mode`, `temperature`, `permission`, plus iter's `kind` = plan, execute, fix or investigate, which says how the work is set up; then the prompt). Files are `instructions/<name>.md`; the nearest wins: the project's notes, its organization's notes, iter's config folder, then the four built in. Code: `src/agent/instruction.rs` (files), `src/web/instructions.rs` (pages).

- `/instructions`: the global list (a table like the tasks one; the first row "New instruction…" opens a pop-up (`?new=instruction`, like New project): name, set up like, what it is for). Columns: the instruction (its description to the right of the name, shown on the row under the pointer, always on touch screens) and how many agents were given it.
- The project's **Instructions** tab: the same list as it applies to the project (project, organization, global, built in). A new instruction there is the project's own.
- `/instruction/{name}[?project=<id>]`: three tabs: **Info** (read-only: description, prompt, details aside: source, kind, model, mode, temperature, permission, file, last edited), **Agents** (every run that was given it: agent, task, state; a run from before instructions existed counts for its kind) and **Settings**, always last (`?tab=settings`; the old `?edit=1` opens it too): the name, and the text, front matter and prompt together, as a form; "Use the inherited one" removes the file at that scope. No Edit button.
- Edit also changes the **name** (the file name; not for the built-in ones, nor to a built-in name, nor to one that exists at that scope): the file at that scope moves, and the page of the new name opens. Agents already run keep the old name recorded, so they count under it.
- On the lists the description of an instruction shows only on the row under the pointer (or focus); touch screens always show it.
- Saving is refused when the front matter does not read, the prompt is empty, or the prompt has a placeholder iter does not know. `POST /instructions` creates, `POST /instruction/{name}` saves or resets.
- The New agent form offers every instruction that applies to the task's project; the model starts as the instruction's `model` when the harness offers it.

New instruction is a pop-up asking for a name, a description and the prompt (it starts as the `develop` prompt; the instruction is made a `develop` one, Settings changes that). The Settings tab is one field each: name (not for the built-in ones), set up like (kind; not for the built-in ones), description, model (with the models offered), mode, temperature, permission (YAML) and prompt. Saving rebuilds the file (`instruction::compose`): the front matter has the fields that are not empty, keys the form does not know stay as they were, then the prompt.
