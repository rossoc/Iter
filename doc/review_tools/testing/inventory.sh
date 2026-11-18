#!/bin/sh
# Count #[test] per source file and list files with none. Run from the repo root.
cd "$(dirname "$0")/../../.." || exit 1
printf '%s\n' "tests  file"
find src -name '*.rs' | while read -r f; do
  n=$(grep -c '#\[test\]' "$f")
  printf '%5d  %s\n' "$n" "$f"
done | sort -rn
printf '\nTotal #[test]: %s in %s files\n' "$(grep -r '#\[test\]' src --include='*.rs' | wc -l | tr -d ' ')" "$(find src -name '*.rs' | wc -l | tr -d ' ')"
printf 'Files with zero tests: %s\n' "$(find src -name '*.rs' | while read -r f; do grep -q '#\[test\]' "$f" || echo "$f"; done | wc -l | tr -d ' ')"
printf '#[ignore]: %s\n' "$(grep -rn '#\[ignore' src | wc -l | tr -d ' ')"
