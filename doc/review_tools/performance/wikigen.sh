#!/usr/bin/env bash
# Fills the org-1 wiki of a seeded store with N well-formed pages (ideas, concepts, entities) that
# link each other, and puts the notes folder under git so the review path runs `git status`.
#   wikigen.sh <store> <pages>
set -euo pipefail
S=${1:?store}; N=${2:?pages}
W="$S/notes/org-1/wiki"; TODAY=$(date +%F)
rm -rf "$W/ideas" "$W/concepts" "$W/entities" "$W/syntheses"; mkdir -p "$W/ideas" "$W/concepts" "$W/entities"
for ((i=1;i<=N;i++)); do
  case $((i % 3)) in 0) f=ideas; c=Idea;; 1) f=concepts; c=Concept;; 2) f=entities; c=Entity;; esac
  prev=$(( i > 1 ? i - 1 : N )); case $((prev % 3)) in 0) pf=ideas;; 1) pf=concepts;; 2) pf=entities;; esac
  cat > "$W/$f/page-$i.md" <<MD
---
description: $c number $i about retries, backoff, jitter and the platform; words w$((i % 50)) and w$((i % 7)).
tags: [t$((i % 20)), net$((i % 5))]
confidence: medium
updated: ${TODAY}T09:00:00
source: [$TODAY]
---
# Page $i

Body of page $i. Relates to [[${pf}/page-${prev}]] and [[concepts/backoff]]; see also [[entities/platform]].
MD
done
cd "$S/notes/org-1" && { [ -d .git ] || git init -q; git add -A >/dev/null; git -c user.email=r@x -c user.name=r commit -qm "wiki $N" >/dev/null 2>&1 || true; }
echo "pages: $(find "$W" -name '*.md' | wc -l | tr -d ' ')"
