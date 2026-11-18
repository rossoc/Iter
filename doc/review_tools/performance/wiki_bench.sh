#!/usr/bin/env bash
# Times the wiki paths at 0, 200 and 2000 pages in the org-1 wiki of <store> served at <base-url>.
#   wiki_bench.sh <store> <iter-binary> <base-url>
set -u
S=${1:?}; BIN=${2:?}; BASE=${3:?}; HERE=$(cd "$(dirname "$0")" && pwd)
export XDG_CONFIG_HOME=$S ITER_PLATFORM_KEY=dummy; unset TMUX
N=7
med() { printf '%s\n' "$@" | sort -g | sed -n "$(( (N+1)/2 ))p"; }
cli() { local label=$1; shift; local t=(); for ((i=0;i<N;i++)); do local a=$(python3 -c 'import time;print(time.time())'); "$@" >/dev/null 2>&1; local b=$(python3 -c 'import time;print(time.time())'); t+=("$(echo "($b-$a)*1000" | bc -l)"); done; printf '  %-40s %8.1f ms\n' "$label" "$(med "${t[@]}")"; }
url() { local t=(); local out; for ((i=0;i<N;i++)); do out=$(curl -s -o /dev/null -w '%{http_code} %{size_download} %{time_total}' "$BASE$1"); t+=("$(echo "${out##* }*1000" | bc -l)"); done; printf '  %-40s %8.1f ms  (%s)\n' "$1" "$(med "${t[@]}")" "${out% *}"; }
for pages in 0 200 2000; do
  if [ "$pages" = 0 ]; then rm -rf "$S/notes/org-1/wiki/ideas" "$S/notes/org-1/wiki/concepts/page-"* "$S/notes/org-1/wiki/entities/page-"*; (cd "$S/notes/org-1" && { [ -d .git ] || git init -q; git add -A >/dev/null; git -c user.email=r@x -c user.name=r commit -qm base >/dev/null 2>&1 || true; }); else bash "$HERE/wikigen.sh" "$S" "$pages" >/dev/null; fi
  echo "== pages=$pages ($(find "$S/notes/org-1/wiki" -name '*.md' | wc -l | tr -d ' ') md files)"
  cli "wiki find --org org-1"      "$BIN" wiki find --org org-1 backoff retry
  cli "wiki find --task p1/t1"     "$BIN" wiki find --task project-1/task-1 backoff retry
  cli "wiki show --org org-1"      "$BIN" wiki show --org org-1 concepts/backoff
  cli "wiki check --org org-1"     "$BIN" wiki check --org org-1
  cli "wiki check --task p1/t1"    "$BIN" wiki check --task project-1/task-1
  cli "wiki pending --project p1"  "$BIN" wiki pending --project project-1
  cli "wiki export --org org-1"    "$BIN" wiki export --org org-1
  cli "session start+stop p1/t2 (ingest::auto)" bash -c "'$BIN' session start project-1/task-2; '$BIN' session stop project-1/task-2"
  url "/task/1?tab=agent"
  url "/task/1?tab=agent&new=agent"
  url "/task/1?tab=wiki"
  url "/task/1"
done
