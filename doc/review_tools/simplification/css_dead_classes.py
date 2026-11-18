#!/usr/bin/env python3
"""Lists every class selector in the .css files under ROOT and how often each
class name appears in .rs/.js files (as a word, inside quotes or class lists).
Usage: css_dead_classes.py CSS_ROOT CODE_ROOT   (classes with 0 uses first, then single-use ones)"""
import os, re, sys
from collections import defaultdict
root, code_root = sys.argv[1], sys.argv[2]
css, code = {}, []
for dp, dn, fn in os.walk(root):
    for f in fn:
        if f.endswith('.css'): css[os.path.join(dp, f)] = open(os.path.join(dp, f), encoding='utf-8').read()
for dp, dn, fn in os.walk(code_root):
    for f in fn:
        if f.endswith(('.rs', '.js')) and not f.startswith('tests'):
            p = os.path.join(dp, f); code.append((p, open(p, encoding='utf-8', errors='replace').read()))
defs = defaultdict(set)
for p, text in css.items():
    text = re.sub(r'/\*.*?\*/', '', text, flags=re.S)
    # selectors: everything before a `{` that is not inside a block of declarations
    for sel in re.findall(r'([^{}]+)\{', text):
        if sel.strip().startswith('@'): continue
        for cls in re.findall(r'\.(-?[_a-zA-Z][_a-zA-Z0-9-]*)', sel):
            defs[cls].add(os.path.relpath(p, root))
# also classes used by JS (classList.add / toggle / className) count as uses
uses = defaultdict(list)
for cls in defs:
    pat = re.compile(r'(?<![\w-])' + re.escape(cls) + r'(?![\w-])')
    for p, text in code:
        n = len(pat.findall(text))
        if n: uses[cls].append((os.path.relpath(p, code_root), n))
# classes also used from other css only (e.g. set by JS via a variable) are reported too
dead = sorted(c for c in defs if not uses[c])
print(f'css files={len(css)} classes defined={len(defs)} with no .rs/.js use={len(dead)}')
for c in dead:
    print(f'  .{c:28s} defined in {", ".join(sorted(defs[c]))}')
print('\nclasses used only in one place (candidates for inlining or review):')
for c in sorted(defs):
    if uses[c] and sum(n for _, n in uses[c]) == 1:
        print(f'  .{c:28s} {uses[c][0][0]}  defined in {", ".join(sorted(defs[c]))}')
