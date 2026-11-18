#!/usr/bin/env python3
"""Heuristic: `pub`/`pub(crate)` fns, structs, enums, consts, types whose name
occurs nowhere else in the tree (outside its defining line and tests). Usage: unused_pub.py ROOT..."""
import os, re, sys
from collections import Counter
files = {}
for root in sys.argv[1:]:
    for dp, dn, fn in os.walk(root):
        for f in fn:
            if f.endswith('.rs'):
                p = os.path.join(dp, f); files[p] = open(p, encoding='utf-8', errors='replace').read()
defs = []
for p, text in files.items():
    if p.endswith('tests.rs'): continue
    in_tests = False
    for i, l in enumerate(text.split('\n'), 1):
        if re.match(r'\s*#\[cfg\(test\)\]', l): in_tests = True
        if in_tests: continue
        m = re.match(r'\s*pub(\([^)]*\))?\s+(async\s+)?(fn|struct|enum|const|static|type|trait|mod)\s+([A-Za-z_]\w*)', l)
        if m: defs.append((p, i, m.group(3), m.group(4)))
words = Counter()
for p, text in files.items():
    if p.endswith('tests.rs'): continue
    # drop test modules
    text = re.split(r'#\[cfg\(test\)\]', text)[0]
    words.update(re.findall(r'[A-Za-z_]\w*', text))
# names also used from macros/ (derive output) or doc links are not counted; report those with count == 1
for p, i, kind, name in sorted(defs):
    if words[name] <= 1:
        print(f'{p}:{i}  {kind} {name}')
