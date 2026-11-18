#!/usr/bin/env bash
# Seeds a throwaway iter store for the 2026-11-02 review.
#
#   seed.sh <scratch-dir> <scale> <iter-binary>
#
# <scratch-dir> becomes $XDG_CONFIG_HOME: the DB is <scratch-dir>/iter/iter.db and the notes live
# under <scratch-dir>/notes. Scale 1 = 2 orgs, 5 projects, 30 tasks, 200 sessions; scale N
# multiplies projects, tasks and sessions by N (orgs: 2*N capped at 8). Re-running wipes the store.
# Never point this at ~/.config/iter.
set -euo pipefail
S=${1:?scratch dir}; SCALE=${2:-1}; BIN=${3:?iter binary}
case "$S" in /Users/*/.config/iter|/Users/*/.config/iter/) echo "refusing to seed the real store" >&2; exit 2;; esac
rm -rf "$S/iter" "$S/notes" "$S/proj"; mkdir -p "$S/iter" "$S/notes" "$S/proj"
# One run creates and migrates the schema.
XDG_CONFIG_HOME="$S" TMUX= ITER_PLATFORM_KEY=dummy "$BIN" project list >/dev/null
DB="$S/iter/iter.db"
ORGS=$(( 2 * SCALE > 8 ? 8 : 2 * SCALE )); PROJECTS=$(( 5 * SCALE )); TASKS=$(( 30 * SCALE )); SESSIONS=$(( 200 * SCALE ))
TODAY=$(date +%F)
sqlite3 "$DB" <<SQL
PRAGMA foreign_keys = ON;
BEGIN;
WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i < $ORGS)
INSERT INTO organizations (name, description, github, tmux, auto_branch, branch_template, default_branch, github_project, notes_path, platform_url)
SELECT 'org-' || i,
       'Organization ' || i || char(10) || 'Second line of the description, with *markdown* and a [link](https://example.com).',
       i % 2, 1, 1, 'feat/{task}', 'main', '', '$S/notes/org-' || i, CASE WHEN i = 1 THEN 'https://platform.example.com' ELSE '' END
FROM n;

WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i < $(( SCALE > 1 ? 2 : 1 )))
INSERT INTO boards (name, description) SELECT 'board-' || i, 'Board ' || i || ' description.' FROM n;

WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i < $PROJECTS)
INSERT INTO projects (name, description, base_path, github, tmux, auto_branch, branch_template, default_branch, github_project, organization_id, board_id, notes_path)
SELECT 'project-' || i,
       CASE WHEN i % 4 = 0 THEN '' ELSE 'Project ' || i || ' does something useful.' || char(10,10) || 'A longer second paragraph so descriptions have body text.' END,
       '$S/proj/project-' || i, i % 2, 1, 1, 'feat/{task}', 'main', '',
       CASE WHEN i % 5 = 0 THEN NULL ELSE ((i - 1) % $ORGS) + 1 END,
       CASE WHEN i % 3 = 0 THEN NULL ELSE ((i - 1) % (SELECT count(*) FROM boards)) + 1 END,
       ''
FROM n;

WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i < $TASKS)
INSERT INTO tasks (project_id, name, description, github_issue, status, branch_prefix, matrix_placed, start_time, duration)
SELECT ((i - 1) % $PROJECTS) + 1,
       CASE WHEN i % 7 = 0 THEN 'A task with a rather long name to see how the layout wraps when names get big ' || i ELSE 'task-' || i END,
       CASE WHEN i % 3 = 0 THEN '' ELSE 'Task ' || i || ': do the thing.' || char(10,10) || '- step one' || char(10) || '- step two' END,
       CASE WHEN i % 4 = 0 THEN i * 11 ELSE NULL END,
       CASE i % 5 WHEN 0 THEN 'done' WHEN 1 THEN 'wip' ELSE 'queue' END,
       CASE WHEN i % 6 = 0 THEN 'fix' ELSE '' END,
       CASE WHEN i % 3 = 0 THEN 1 ELSE 0 END,
       CASE WHEN i % 4 = 1 THEN datetime('$TODAY', '+' || (i % 9 + 7) || ' hours', '-' || (i % 5) || ' days')
            WHEN i % 4 = 2 THEN datetime('$TODAY', '+' || (i % 10 + 8) || ' hours')
            ELSE NULL END,
       CASE WHEN i % 4 IN (1, 2) THEN (i % 4 + 1) * 30 ELSE NULL END
FROM n;

INSERT OR IGNORE INTO tags (name, color, description) VALUES ('bug', '#ef4444', 'Something is broken.'), ('docs', '#10b981', 'Writing, not code.'), ('research', '#8b5cf6', 'Find out before deciding.');

INSERT OR IGNORE INTO task_tags (task_id, tag_id)
SELECT t.id, ((t.id * 7) % (SELECT count(*) FROM tags)) + 1 FROM tasks t WHERE t.id % 2 = 0;
INSERT OR IGNORE INTO task_tags (task_id, tag_id)
SELECT t.id, ((t.id * 3) % (SELECT count(*) FROM tags)) + 1 FROM tasks t WHERE t.id % 5 = 0;

WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i < $SESSIONS)
INSERT INTO sessions (task_id, start, end, message)
SELECT ((i - 1) % $TASKS) + 1,
       datetime('$TODAY', '-' || (i % 30) || ' days', '+' || (8 + i % 9) || ' hours', '+' || ((i * 13) % 60) || ' minutes'),
       CASE WHEN i = $SESSIONS THEN NULL ELSE datetime('$TODAY', '-' || (i % 30) || ' days', '+' || (8 + i % 9) || ' hours', '+' || ((i * 13) % 60 + 20 + i % 70) || ' minutes') END,
       CASE WHEN i % 3 = 0 THEN 'worked on part ' || i ELSE NULL END
FROM n;

INSERT INTO session_configs (task_id, tmux_session_name, github_branch, worktree_path)
SELECT id, 'project-' || project_id || '/' || name, 'feat/' || name, NULL FROM tasks WHERE status = 'wip' AND id % 2 = 0 AND name NOT LIKE 'A task%';
COMMIT;
SQL

# Notes and a wiki per organization; a few daily notes per project.
for ((o = 1; o <= ORGS; o++)); do
  d="$S/notes/org-$o"; mkdir -p "$d/wiki/concepts" "$d/wiki/entities"
  for back in 0 1 3 9; do
    day=$(date -v-${back}d +%F 2>/dev/null || date -d "-$back day" +%F)
    printf -- '---\nlayout: page\ntitle: %s\ndate: %s\n---\nWhat happened on %s in org-%d.\n\n- met about **task-%d**\n- see [[concepts/backoff]]\n' "$(date -j -f %F "$day" +%d/%m 2>/dev/null || echo "$day")" "$day" "$day" "$o" "$back" > "$d/$day.md"
  done
  cat > "$d/wiki/concepts/backoff.md" <<MD
---
description: Retry with exponential backoff and jitter so clients do not stampede.
tags: [net, retry]
confidence: medium
updated: ${TODAY}T09:00:00
source: [$TODAY]
---
# Backoff

Wait longer after each failure; add jitter. Related: [[entities/platform]].
MD
  cat > "$d/wiki/entities/platform.md" <<MD
---
description: The time platform org-$o pushes worklogs to.
tags: [platform]
confidence: high
updated: ${TODAY}T09:05:00
---
# Platform

Receives POST /time-entries. See [[concepts/backoff]].
MD
  for ((p = o; p <= PROJECTS; p += ORGS)); do
    pd="$d/project-$p"; mkdir -p "$pd"
    printf -- '---\nlayout: page\ntitle: today\ndate: %s\n---\nProject %d note for today.\n' "$TODAY" "$p" > "$pd/$TODAY.md"
  done
done
sqlite3 "$DB" "SELECT 'orgs', count(*) FROM organizations UNION ALL SELECT 'boards', count(*) FROM boards UNION ALL SELECT 'projects', count(*) FROM projects UNION ALL SELECT 'tasks', count(*) FROM tasks UNION ALL SELECT 'sessions', count(*) FROM sessions UNION ALL SELECT 'task_tags', count(*) FROM task_tags;"
