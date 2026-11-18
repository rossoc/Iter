#!/usr/bin/env bash
# Median-of-5 wall time, bytes and status for every route of the brief's route map.
#   routes.sh <base-url> <label>
# Prints: route  status  bytes  median_ms  (min_ms)
# Ids: the seed makes org 1, project 1, task 1, board 1 exist at every scale; the notes date is today.
set -u
BASE=${1:?base url}; LABEL=${2:-}
TODAY=$(date +%F)
N=${RUNS:-5}
ROUTES=(
  "/" "/settings"
  "/org/1" "/org/1?tab=notes" "/org/1?tab=tasks" "/org/1?tab=report" "/org/1?share=1" "/org/1/edit" "/org/1/notes/$TODAY" "/org/1/platform-activities"
  "/project/1" "/project/1?tab=notes" "/project/1?tab=tasks" "/project/1?tab=report" "/project/1/edit" "/project/1/task/new" "/project/1/notes/$TODAY"
  "/task/1" "/task/1?tab=sessions" "/task/1?tab=notes" "/task/1?tab=agent" "/task/1?tab=agent&new=agent" "/task/1?tab=wiki" "/task/1/edit"
  "/agents" "/boards" "/board/1" "/board/1?date=$TODAY" "/board/1?pick=1" "/board/1/matrix" "/board/1/info" "/board/1/edit"
  "/instructions" "/instruction/plan"
  "/search?q=" "/search?q=task" "/search?q=org-1/task-1" "/search?q=is:wip" "/search?q=note"
  "/style.css" "/board.js" "/org.js" "/platform.js" "/favicon.ico"
)
for r in "${ROUTES[@]}"; do
  times=(); code=; bytes=
  for ((i=0;i<N;i++)); do
    out=$(curl -s -o /dev/null -w '%{http_code} %{size_download} %{time_total}' "$BASE$r")
    code=${out%% *}; rest=${out#* }; bytes=${rest%% *}; t=${rest#* }
    times+=("$t")
  done
  sorted=$(printf '%s\n' "${times[@]}" | sort -g)
  med=$(echo "$sorted" | sed -n 3p); min=$(echo "$sorted" | sed -n 1p)
  printf '%s\t%s\t%s\t%s\t%.1f\t%.1f\n' "$LABEL" "$r" "$code" "$bytes" "$(echo "$med*1000" | bc -l)" "$(echo "$min*1000" | bc -l)"
done
