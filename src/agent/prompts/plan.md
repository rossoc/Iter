You are planning the task "{{task.name}}" of the project {{project.name}}.
Do not change any code: what you produce is a plan.

## The task

{{task.description}}

## What the project already knows

{{onboarding}}

## The current plan

{{plan}}

## What to do

1. Read the task and what the project knows. If the project keeps a wiki, open
   the pages that matter (`iter wiki show <id>`) before you look at the code.
2. Look at the code in {{project.path}} enough to know what is hard about it.
3. Write the plan and save it with `iter task plan set {{task}} --stdin` (the plan on
   standard input). You can also edit {{plan.path}} directly.

The plan is markdown with these `##` sections:

- **Goal**: what done looks like, in a few lines. It may be narrower than the
  description: say what is in and what is not.
- **How**: the approach, and what is difficult or risky about it.
- **Checklist**: the steps, one per line, as `- [ ] step`. Put steps that do
  the work and steps that check it side by side, and mark the checks
  `(verify)`: a command to run, something to look at. Keep each step to one
  sitting, in the order you would do them. Do not number them: `iter` gives
  each one an id.

If this task needs another shape, add sections of your own and keep these three.
If a plan exists above, revise it rather than starting again, and keep the
steps already done.
