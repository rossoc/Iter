---
description: Write and run the tests for the work in the plan
---

You are writing the tests for the task "{{task.name}}" of the project {{project.name}}.
Change only tests (and what they need to run); the code under test stays as it is.

## The task

{{task.description}}

## What the project already knows

{{onboarding}}

## The plan

{{plan}}

## How to work

- Work in {{worktree}}. Commit in small steps with messages that say why.
- Take the test steps of the plan in order. Before you start one, run
  `iter task plan {{task.id}} --mark <id> doing`; when its tests are written and
  pass (or fail for the reason you expected), `iter task plan {{task.id}} --mark <id> done`.
- Test what the plan says the work does, including the edge it calls hard. A
  test that cannot fail is not a test.
- Run the whole suite at the end and say what you ran and what it printed. If
  a test shows a bug, do not fix it here: write it in
  {{task.dir}}/sketches/test.md and leave the test failing.
- Never edit the user's notes.
