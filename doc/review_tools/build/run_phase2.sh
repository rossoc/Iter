#!/bin/zsh
export XDG_CONFIG_HOME=/private/tmp/claude-501/-Users-carlorosso--config-iter--iter-worktrees-review/f9634ce8-ea6d-484c-8703-6013688875b7/scratchpad/test-home ITER_PLATFORM_KEY=dummy; unset TMUX
E=/Users/carlorosso/.config/iter/.iter-worktrees/review/doc/review_tools/build/exp.sh; D=/Users/carlorosso/.config/iter/.iter-worktrees/review/doc/review_tools/build/dev_exp.sh
C=/private/tmp/claude-501/-Users-carlorosso--config-iter--iter-worktrees-review/f9634ce8-ea6d-484c-8703-6013688875b7/scratchpad/build-copy; T=/private/tmp/claude-501/-Users-carlorosso--config-iter--iter-worktrees-review/f9634ce8-ea6d-484c-8703-6013688875b7/scratchpad/build-target
cd $C
echo "=== A: incremental=false (cargo default release)"
$E release-noincr $C $T/noincr --config 'profile.release.incremental=false'
echo "=== 7: dist profile (fat lto, cgu=1, strip, no incremental)"
$E dist-profile $C $T/dist --profile dist
echo "=== 7alt: dist with opt-level=s"
$E dist-opt-s $C $T/dists --profile dist --config 'profile.dist.opt-level="s"'
rm -rf $T/dists $T/noincr
echo "=== D: time-passes on Iter crate (non-incremental), reusing base deps"
cd $C; touch src/main.rs; CARGO_TARGET_DIR=$T/base RUSTC_BOOTSTRAP=1 cargo rustc --release --config 'profile.release.incremental=false' -- -Z time-passes > $T/time-passes.log 2>&1; grep -c Compiling $T/time-passes.log
echo "=== clap_complete without unstable-dynamic: cargo check"
sed -i '' 's|clap_complete = { version = "4.6.9", features = \["unstable-dynamic"\] }|clap_complete = "4.6.9"|' Cargo.toml; grep -n clap_complete Cargo.toml
CARGO_TARGET_DIR=$T/clapcheck cargo check --release > $T/clapcheck.log 2>&1; echo rc=$?; grep -E '^error' $T/clapcheck.log | sort | uniq -c | sort -rn | head -20
cp Cargo.toml.orig Cargo.toml; cp Cargo.lock.orig Cargo.lock
echo "=== trim: rusqlite system sqlite"
sed -i '' 's|rusqlite = { version = "0.40.2", features = \["bundled"\] }|rusqlite = { version = "0.40.2" }|' Cargo.toml; grep -n rusqlite Cargo.toml
echo crates: $(cargo tree -e normal --prefix none | sort -u | wc -l)
$E trim-sys-sqlite $C $T/syssqlite
cp Cargo.toml.orig Cargo.toml; cp Cargo.lock.orig Cargo.lock
echo "=== trim: ureq native-tls"
sed -i '' 's|^ureq = "3.5.0"|ureq = { version = "3.5.0", default-features = false, features = ["native-tls", "gzip"] }|' Cargo.toml; grep -n '^ureq' Cargo.toml
echo crates: $(cargo tree -e normal --prefix none | sort -u | wc -l)
$E trim-native-tls $C $T/nativetls
cp Cargo.toml.orig Cargo.toml; cp Cargo.lock.orig Cargo.lock
echo "=== trim: regex-lite (src edit in copy) -- check only"
sed -i '' 's|^regex = "1"|regex-lite = "0.1"|' Cargo.toml; sed -i '' 's|use regex::RegexBuilder;|use regex_lite::RegexBuilder;|' src/wiki/retrieve.rs
echo crates: $(cargo tree -e normal --prefix none | sort -u | wc -l)
CARGO_TARGET_DIR=$T/clapcheck cargo check --release > $T/regexlite.log 2>&1; echo rc=$?; grep -E '^error' -A5 $T/regexlite.log | head -20
cp Cargo.toml.orig Cargo.toml; cp Cargo.lock.orig Cargo.lock; sed -i '' 's|use regex_lite::RegexBuilder;|use regex::RegexBuilder;|' src/wiki/retrieve.rs
echo "=== 7b: dist + sys sqlite + native-tls"
sed -i '' 's|rusqlite = { version = "0.40.2", features = \["bundled"\] }|rusqlite = { version = "0.40.2" }|; s|^ureq = "3.5.0"|ureq = { version = "3.5.0", default-features = false, features = ["native-tls", "gzip"] }|' Cargo.toml
echo crates: $(cargo tree -e normal --prefix none | sort -u | wc -l)
$E dist+trims $C $T/disttrim --profile dist
cp Cargo.toml.orig Cargo.toml; cp Cargo.lock.orig Cargo.lock
rm -rf $T/syssqlite $T/nativetls $T/disttrim $T/clapcheck
echo "=== 8: dev profile rows"
$D dev-baseline $C $T/dev0
$D dev-ltonly-depsnodebug $C $T/dev1 --config 'profile.dev.debug="line-tables-only"' --config 'profile.dev.package."*".debug=false'
$D dev-ltonly-depsnodebug-opt1 $C $T/dev2 --config 'profile.dev.debug="line-tables-only"' --config 'profile.dev.package."*".debug=false' --config 'profile.dev.package."*".opt-level=1'
rm -rf $T/dev0 $T/dev1 $T/dev2
echo "=== udeps (nightly)"
CARGO_TARGET_DIR=$T/udeps cargo +nightly udeps --release --all-targets > $T/udeps.log 2>&1; echo rc=$?; tail -25 $T/udeps.log
echo "=== tests on dist-like profile (fat lto, cgu1, strip)"
t0=$(date +%s); CARGO_TARGET_DIR=$T/disttest cargo test --release --config 'profile.release.lto="fat"' --config 'profile.release.codegen-units=1' --config 'profile.release.strip=true' --config 'profile.release.incremental=false' > $T/disttest.log 2>&1; echo rc=$? took $(( $(date +%s)-t0 ))s; grep -E '^test result' $T/disttest.log
echo PHASE2_DONE
