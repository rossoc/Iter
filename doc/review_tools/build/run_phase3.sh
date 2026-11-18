#!/bin/zsh
E=/Users/carlorosso/.config/iter/.iter-worktrees/review/doc/review_tools/build/exp.sh; C=/private/tmp/claude-501/-Users-carlorosso--config-iter--iter-worktrees-review/f9634ce8-ea6d-484c-8703-6013688875b7/scratchpad/build-copy; T=/private/tmp/claude-501/-Users-carlorosso--config-iter--iter-worktrees-review/f9634ce8-ea6d-484c-8703-6013688875b7/scratchpad/build-target
DIST=(--config 'profile.release.lto="fat"' --config 'profile.release.codegen-units=1' --config 'profile.release.strip=true' --config 'profile.release.incremental=false')
cd $C; cp Cargo.toml.orig Cargo.toml; cp Cargo.lock.orig Cargo.lock
$E dist $C $T/dist "${DIST[@]}"; rm -rf $T/dist
$E dist-opt-s $C $T/dists "${DIST[@]}" --config 'profile.release.opt-level="s"'; rm -rf $T/dists
echo "=== trim: ureq native-tls (+der pin)"
sed -i '' 's|^ureq = "3.5.0"|ureq = { version = "3.5.0", default-features = false, features = ["native-tls", "gzip"] }\nder = "0.8.0-rc.8"|' Cargo.toml; grep -n '^ureq\|^der' Cargo.toml
echo crates: $(cargo tree -e normal --prefix none | sort -u | wc -l); cargo tree -e normal --prefix none | sort -u > $T/nativetls.crates
$E trim-native-tls $C $T/nativetls; rm -rf $T/nativetls
echo "=== combo: dist + sys sqlite + native-tls"
sed -i '' 's|rusqlite = { version = "0.40.2", features = \["bundled"\] }|rusqlite = { version = "0.40.2" }|' Cargo.toml
echo crates: $(cargo tree -e normal --prefix none | sort -u | wc -l)
$E dist+sys-sqlite+native-tls $C $T/combo "${DIST[@]}"; cp $T/combo/release/Iter $T/combo.bin; rm -rf $T/combo
cp Cargo.toml.orig Cargo.toml; cp Cargo.lock.orig Cargo.lock
echo PHASE3_DONE
