#!/usr/bin/env bash
# Adds realistic filesystem content to a seeded store (seed.sh leaves the folders almost empty):
#   fsload.sh <store> [live-pid]
# - 365 daily notes per organization, 60 per project (what /search and the Notes tabs read)
# - 3 agent runs per task that has a notes folder: #1 done, #2 failed, #3 running with <live-pid>
#   when given (else done), so `shown_status` has processes to probe. Idempotent.
set -euo pipefail
S=${1:?store}; PID=${2:-}
DB="$S/iter/iter.db"
TODAY=$(date +%F)
note() { # dir day idx
  printf -- '---\nlayout: page\ntitle: Day %s\ndate: %s\n---\nNotes of %s: met about **task-%s**, decided on retries and backoff.\n\n- see [[concepts/backoff]]\n- follow up on the platform\n' "$3" "$2" "$2" "$3" > "$1/$2.md"
}
sqlite3 "$DB" "SELECT notes_path FROM organizations WHERE notes_path <> ''" | while read -r d; do
  mkdir -p "$d"
  for ((b=0;b<365;b++)); do day=$(date -v-${b}d +%F); [ -f "$d/$day.md" ] || note "$d" "$day" "$b"; done
done
sqlite3 "$DB" "SELECT p.id, p.name, o.notes_path FROM projects p JOIN organizations o ON o.id = p.organization_id WHERE o.notes_path <> ''" | while IFS='|' read -r pid name root; do
  d="$root/$name"; mkdir -p "$d"
  for ((b=0;b<60;b++)); do day=$(date -v-${b}d +%F); [ -f "$d/$day.md" ] || note "$d" "$day" "$b"; done
  sqlite3 "$DB" "SELECT id FROM tasks WHERE project_id = $pid" | while read -r tid; do
    r="$d/tasks/$tid/runs"; mkdir -p "$r"
    for n in 1 2 3; do
      case $n in 1) st=done;; 2) st=failed;; 3) if [ -n "$PID" ]; then st=running; else st=done; fi;; esac
      printf 'n: %d\nagent: claude\nmodel: opus\neffort: medium\nkind: plan\ninstruction: plan\nstatus: %s\nstarted: %s 09:0%d:00\n' "$n" "$st" "$TODAY" "$n" > "$r/$n.yaml"
      if [ "$st" = running ]; then printf 'pid: %s\n' "$PID" >> "$r/$n.yaml"; else printf 'ended: %s 09:1%d:00\nexit_code: 0\n' "$TODAY" "$n" >> "$r/$n.yaml"; fi
      printf 'prompt of run %d\n' "$n" > "$r/$n.prompt.md"; printf 'log line\n' > "$r/$n.log"
    done
  done
done
echo "notes: $(find "$S/notes" -name '20*.md' | wc -l | tr -d ' ')  runs: $(find "$S/notes" -name '*.yaml' -path '*/runs/*' | wc -l | tr -d ' ')"
