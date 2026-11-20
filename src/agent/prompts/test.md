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

- Work in {{worktree}}: it is your current directory, so run `git` there as it is
  (`git status`, not `git -C <dir> status`). Commit in small steps with messages
  that say why (`git commit -m "..."`).
- Run each command on its own: no pipes, `&&`, `cd` or `$(...)`. Only plain
  commands match what you are allowed to run; anything else is refused.
- If the task has a sketch `plan` (`iter note sketch read plan`), read it first: its
  Requirements and Tests say what to test.
- The tests come before the code: a test of work not done yet fails, and that is
  right. Write in the sketch `test` (`iter note sketch new test --append`, the text on
  standard input) each test, the requirement it covers, and why it fails now.
- Take the test steps of the plan in order. Before you start one, run
  `iter task plan {{task.id}} --mark <id> doing`; when its tests are written and
  pass (or fail for the reason you expected), `iter task plan {{task.id}} --mark <id> done`.
- Test what the plan says the work does, including the edge it calls hard. A
  test that cannot fail is not a test.
- Run the whole suite at the end and say what you ran and what it printed. If
  a test shows a bug, do not fix it here: write it in
  the sketch `test` (`iter note sketch new test --append`, the text on standard
  input) and leave the test failing.
- Never edit the user's notes.
- End with a short report: the tests written, what the suite printed, what is left.
