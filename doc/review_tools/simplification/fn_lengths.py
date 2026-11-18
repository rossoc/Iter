#!/usr/bin/env python3
"""Lists Rust functions longer than N lines (brace matching, comments/strings ignored naively).
Usage: fn_lengths.py MIN ROOT..."""
import os, re, sys
minimum = int(sys.argv[1]); out = []
for root in sys.argv[2:]:
    for dp, dn, fn in os.walk(root):
        for f in fn:
            if not f.endswith('.rs') or f == 'tests.rs': continue
            p = os.path.join(dp, f); lines = open(p, encoding='utf-8', errors='replace').read().split('\n')
            in_tests = False
            for i, l in enumerate(lines):
                if re.match(r'\s*#\[cfg\(test\)\]', l): in_tests = True
                m = re.match(r'\s*(pub(\([^)]*\))?\s+)?(async\s+)?fn\s+(\w+)', l)
                if not m or in_tests: continue
                depth = 0; started = False
                for j in range(i, len(lines)):
                    s = re.sub(r'"(?:\\.|[^"\\])*"', '""', lines[j]); s = s.split('//')[0]
                    depth += s.count('{') - s.count('}')
                    if '{' in s: started = True
                    if started and depth <= 0:
                        n = j - i + 1
                        if n >= minimum: out.append((n, f'{p}:{i+1}', m.group(4)))
                        break
out.sort(reverse=True)
for n, loc, name in out: print(f'{n:4d}  {loc}  {name}')
