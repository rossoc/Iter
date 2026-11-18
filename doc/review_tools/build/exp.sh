#!/bin/zsh
# Usage: exp.sh <label> <copy-dir> <target-dir> [cargo build args...]
# Clean release build (wall), incremental rebuild after `touch src/main.rs`, size, stripped size.
set -u
label=$1; copy=$2; tgt=$3; shift 3
cd "$copy" || exit 1
export CARGO_TARGET_DIR=$tgt
rm -rf "$tgt"; mkdir -p "$(dirname "$tgt")"; setopt nullglob
t0=$(date +%s.%N); cargo build --release "$@" >"$tgt.clean.log" 2>&1; rc=$?; t1=$(date +%s.%N)
clean=$(printf '%.1f' $(echo "$t1-$t0" | bc))
touch src/main.rs
t0=$(date +%s.%N); cargo build --release "$@" >"$tgt.incr.log" 2>&1; t1=$(date +%s.%N)
incr=$(printf '%.1f' $(echo "$t1-$t0" | bc))
bin=$(ls "$tgt"/release/Iter "$tgt"/*/release/Iter 2>/dev/null | head -1)
size=$(stat -f %z "$bin" 2>/dev/null || echo 0)
cp "$bin" "$tgt.stripped" 2>/dev/null && strip "$tgt.stripped" 2>/dev/null
ssize=$(stat -f %z "$tgt.stripped" 2>/dev/null || echo 0)
echo "| $label | rc=$rc | clean ${clean}s | incr ${incr}s | $((size/1024/1024)).$(( (size%1048576)*10/1048576 )) MB ($size) | stripped $((ssize/1024/1024)).$(( (ssize%1048576)*10/1048576 )) MB ($ssize) | args: $* |" | tee -a "$(dirname $tgt)/results.md"
