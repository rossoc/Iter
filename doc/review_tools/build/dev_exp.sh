#!/bin/zsh
# Usage: dev_exp.sh <label> <copy-dir> <target-dir> [cargo args...]
# Clean debug build, incremental after touch, then `cargo test --no-run` (wall seconds), plus target dir size.
set -u; setopt nullglob
label=$1; copy=$2; tgt=$3; shift 3
cd "$copy" || exit 1
export CARGO_TARGET_DIR=$tgt
rm -rf "$tgt"; mkdir -p "$(dirname "$tgt")"
t0=$(date +%s.%N); cargo build "$@" >"$tgt.clean.log" 2>&1; rc=$?; t1=$(date +%s.%N)
clean=$(printf '%.1f' $(echo "$t1-$t0" | bc))
touch src/main.rs
t0=$(date +%s.%N); cargo build "$@" >"$tgt.incr.log" 2>&1; t1=$(date +%s.%N)
incr=$(printf '%.1f' $(echo "$t1-$t0" | bc))
t0=$(date +%s.%N); cargo test --no-run "$@" >"$tgt.test.log" 2>&1; trc=$?; t1=$(date +%s.%N)
tnr=$(printf '%.1f' $(echo "$t1-$t0" | bc))
touch src/main.rs
t0=$(date +%s.%N); cargo test --no-run "$@" >"$tgt.test2.log" 2>&1; t1=$(date +%s.%N)
tnr2=$(printf '%.1f' $(echo "$t1-$t0" | bc))
bin=$(ls "$tgt"/debug/Iter 2>/dev/null | head -1); size=$(stat -f %z "$bin" 2>/dev/null || echo 0)
du=$(du -sh "$tgt" | cut -f1)
echo "| $label | rc=$rc/$trc | dev clean ${clean}s | dev incr ${incr}s | test --no-run (after build) ${tnr}s | test --no-run incr ${tnr2}s | debug bin $((size/1024/1024)) MB | target $du | args: $* |" | tee -a "$(dirname $tgt)/results.md"
