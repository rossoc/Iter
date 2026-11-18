#!/bin/zsh
E=/Users/carlorosso/.config/iter/.iter-worktrees/review/doc/review_tools/build/exp.sh
C=/private/tmp/claude-501/-Users-carlorosso--config-iter--iter-worktrees-review/f9634ce8-ea6d-484c-8703-6013688875b7/scratchpad/build-copy
T=/private/tmp/claude-501/-Users-carlorosso--config-iter--iter-worktrees-review/f9634ce8-ea6d-484c-8703-6013688875b7/scratchpad/build-target
$E baseline $C $T/base
$E strip $C $T/strip --config 'profile.release.strip=true'
$E lto-thin $C $T/thin --config 'profile.release.lto="thin"'
$E lto-fat-cgu1 $C $T/fat --config 'profile.release.lto="fat"' --config 'profile.release.codegen-units=1'
$E opt-s $C $T/opts --config 'profile.release.opt-level="s"'
$E opt-z $C $T/optz --config 'profile.release.opt-level="z"'
$E no-default-features $C $T/nodef --no-default-features
echo PROFILES_DONE
