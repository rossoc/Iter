#!/usr/bin/env bash
# Reproductions for doc/review_2026-11-02_correctness_core.md.
#   repro.sh <scratch-home> <iter-binary>
# <scratch-home> becomes $XDG_CONFIG_HOME (never ~/.config/iter). Uses ed.sh (beside this file) as a
# scripted editor: ITER_ED_SED is a sed expression applied to the buffer iter opens.
set -u
H=${1:?scratch home}; BIN=${2:?iter binary}; HERE=$(cd "$(dirname "$0")" && pwd)
case "$H" in /Users/*/.config/iter|/Users/*/.config/iter/) echo "refusing the real store" >&2; exit 2;; esac
rm -rf "$H"; mkdir -p "$H/iter"; printf 'editor: %s\n' "$HERE/ed.sh" > "$H/iter/iter.yaml"
unset TMUX; export XDG_CONFIG_HOME="$H" ITER_PLATFORM_KEY=dummy
SRC="$H/cc-src"; mkdir -p "$SRC/sub"; echo hello > "$SRC/README.md"; echo x > "$SRC/sub/file.txt"

echo "## C1 clone into a directory inside the source (expect: ENAMETOOLONG, hundreds of nested dirs)"
ITER_ED_SED='s|^tmux: true|tmux: false|' "$BIN" new "$SRC"
ITER_ED_SED="s|^name:.*|name: clone1|; s|^base_path:.*|base_path: $SRC/sub/clone|" perl -e 'alarm 20; exec @ARGV' "$BIN" clone cc-src
echo "dirs under sub/clone: $(find "$SRC/sub/clone" -type d | wc -l)"

echo "## C2 tmux hook with a session name starting with '-' (expect: clap error, exit 2)"
"$BIN" internal hook session-closed "-web/a"; echo "exit=$?"

echo "## C5 task name with trailing space is stored untrimmed (web trims)"
ITER_ED_SED="s|^name:.*|name: 'task '|" "$BIN" task new cc-src
sqlite3 "$H/iter/iter.db" "select id, '[' || name || ']' from tasks"
"$BIN" task info cc-src/task; echo "exit=$?"

echo "## C6 a session crossing midnight is counted whole on its start day"
sqlite3 "$H/iter/iter.db" "insert into sessions(task_id,start,end) select id,'2026-10-20 23:30:00','2026-10-21 01:30:00' from tasks limit 1"
"$BIN" project info cc-src --date 2026-10-20 -f yaml | grep total_hhmm | head -1
"$BIN" project info cc-src --date 2026-10-21 -f yaml | grep total_hhmm | head -1

echo "## C3 task done without --save deletes a branch holding an unmerged commit"
R="$H/cc-repo"; mkdir -p "$R"; git -C "$R" init -q -b main; git -C "$R" -c user.email=a@b -c user.name=t commit -q --allow-empty -m init
ITER_ED_SED="s|^tmux: true|tmux: false|; s|^github: .*|github: true|" "$BIN" new "$R"
ITER_ED_SED="s|^name:.*|name: feat1|" "$BIN" task new cc-repo
"$BIN" session new cc-repo/feat1
WT=$(sqlite3 "$H/iter/iter.db" "select worktree_path from session_configs")
echo work > "$WT/work.txt"; git -C "$WT" add work.txt; git -C "$WT" -c user.email=a@b -c user.name=t commit -q -m unmerged
"$BIN" task done cc-repo/feat1; git -C "$R" branch

echo "## C8 tmux: a bare -t target prefix-matches another session (throwaway server)"
export TMUX_TMPDIR=$(mktemp -d /tmp/itx.XXXX)
tmux new-session -d -s "web/ab" -c /tmp; tmux has-session -t "web/a"; echo "has-session -t web/a exit=$? (0 = matched web/ab)"; tmux kill-server; rm -rf "$TMUX_TMPDIR"

echo "## O1 Organization::check accepts http://localhost:1@evil.example/"
ITER_ED_SED="s|^name:.*|name: evilorg|; s|^platform_url:.*|platform_url: http://localhost:1@evil.example/api|" "$BIN" org new
sqlite3 "$H/iter/iter.db" "select name, platform_url from organizations"
