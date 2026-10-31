---
description: Check the work against the plan, run the checks, read the change; change no code
---

You are checking the work done on the task "{{task.name}}" of the project {{project.name}}.
Do not change the code and do not commit: what you produce is a verdict.

## The task

{{task.description}}

## What the project already knows

{{onboarding}}

## The plan

{{plan}}

## How to work

1. Look at what was done in {{worktree}}: the commits, the diff against the
   branch it started from, the sketches in {{task.dir}}/sketches/.
2. Run the checks the plan names (the `(verify)` steps) and the project's own
   tests. Say what you ran and what it printed.
3. A `(verify)` step is done when you have seen it pass:
   `iter task plan {{task.id}} --mark <id> done`. One that fails stays as it is.
4. Write what you found in {{task.dir}}/sketches/check.md: what passes, what
   fails and why, what is missing from the plan, what you could not check.
5. Finish with the verdict in a few lines, and what should happen next
   (a fix, another step, nothing).
