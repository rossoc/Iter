#!/usr/bin/env bash
# Median wall time (ms) of the CLI commands users hit most, N runs each (hyperfine is not installed).
#   cli.sh <store> <iter-binary> [runs]
set -u
S=${1:?store}; BIN=${2:?bin}; N=${3:-7}
export XDG_CONFIG_HOME=$S ITER_PLATFORM_KEY=dummy; unset TMUX
TODAY=$(date +%F); MONTH_AGO=$(date -v-30d +%F)
run() { # label, cmd...
  local label=$1; shift; local times=()
  for ((i=0;i<N;i++)); do
    local t0=$(python3 -c 'import time;print(time.time())'); "$@" >/dev/null 2>&1; local t1=$(python3 -c 'import time;print(time.time())')
    times+=("$(echo "($t1-$t0)*1000" | bc -l)")
  done
  local med=$(printf '%s\n' "${times[@]}" | sort -g | sed -n "$(( (N+1)/2 ))p")
  printf '%-28s %8.1f ms\n' "$label" "$med"
}
run "baseline python-spawn"      true
run "--version"                  "$BIN" --version
run "project list"               "$BIN" project
run "task list"                  "$BIN" task
run "project info (today)"       "$BIN" project info project-1
run "project info (30 days)"     "$BIN" project info project-1 --from "$MONTH_AGO" --to "$TODAY"
run "org info (today)"           "$BIN" org info org-1
run "org info (30 days)"         "$BIN" org info org-1 --from "$MONTH_AGO" --to "$TODAY"
run "org info worklog (30 days)" "$BIN" org info org-1 --from "$MONTH_AGO" --to "$TODAY" -f worklog
run "session start+stop"         bash -c "'$BIN' session start project-1/task-2; '$BIN' session stop project-1/task-2"
run "notes list --org org-1"     "$BIN" notes list --org org-1
run "notes list --project"       "$BIN" notes list --project project-1
run "wiki find --org org-1"      "$BIN" wiki find --org org-1 backoff retry
run "wiki find --task"           "$BIN" wiki find --task project-1/task-1 backoff retry
run "wiki show --org org-1"      "$BIN" wiki show --org org-1 concepts/backoff
run "wiki check --org org-1"     "$BIN" wiki check --org org-1
run "wiki check --task"          "$BIN" wiki check --task project-1/task-1
run "wiki pending --project"     "$BIN" wiki pending --project project-1
