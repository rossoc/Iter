#!/usr/bin/env python3
"""Clone detector: normalised token windows over .rs/.js/.css files.

Usage: clones.py [--window N] [--min-tokens N] [--ids keep|mask] [--top N] ROOT...

Each line is stripped of comments and whitespace, tokenised; windows of N
consecutive non-empty lines are hashed. By default identifiers are kept and
string/number literals are masked ("S"/"0"). With --ids mask identifiers are
replaced by "ID" too (finds structurally identical code).  Overlapping windows
with the same partner are merged into one clone pair with a line span.
Results: pairs sorted by span length, with file:line-line for both copies.
"""
import os, re, sys, argparse, hashlib
from collections import defaultdict

TOKEN = re.compile(r'"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])*\'|[A-Za-z_][A-Za-z0-9_]*|\d+(?:\.\d+)?|::|->|=>|==|!=|<=|>=|&&|\|\||[^\sA-Za-z0-9_]')

def strip_comments(text, ext):
    # block comments
    text = re.sub(r'/\*.*?\*/', '', text, flags=re.S)
    out = []
    for line in text.split('\n'):
        if ext in ('.rs', '.js'):
            # naive: cut // when not inside a string
            i = 0; instr = None
            while i < len(line):
                c = line[i]
                if instr:
                    if c == '\\': i += 1
                    elif c == instr: instr = None
                elif c in '"\'': instr = c
                elif line.startswith('//', i): line = line[:i]; break
                i += 1
        out.append(line)
    return out

def norm_line(line, mask_ids):
    toks = TOKEN.findall(line)
    res = []
    for t in toks:
        if t[0] in '"\'': res.append('S')
        elif t[0].isdigit(): res.append('0')
        elif mask_ids and re.match(r'[A-Za-z_]', t) and t not in KEYWORDS: res.append('ID')
        else: res.append(t)
    return ' '.join(res)

KEYWORDS = set('''fn let mut pub use mod struct enum impl trait for in if else match return
Self self where as ref move async await loop while break continue const static type dyn
function var const let return if else for while new this class export import default null
undefined true false Some None Ok Err Vec String Option Result'''.split())

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--window', type=int, default=6)
    ap.add_argument('--min-tokens', type=int, default=4, help='skip lines with fewer tokens (braces etc.)')
    ap.add_argument('--ids', choices=['keep', 'mask'], default='keep')
    ap.add_argument('--top', type=int, default=60)
    ap.add_argument('--ext', default='.rs,.js,.css')
    ap.add_argument('--exclude', default='tests.rs,/tests/')
    ap.add_argument('roots', nargs='+')
    a = ap.parse_args()
    exts = set(a.ext.split(','))
    excl = a.exclude.split(',')
    files = []
    for r in a.roots:
        for dp, dn, fn in os.walk(r):
            for f in fn:
                p = os.path.join(dp, f)
                if os.path.splitext(f)[1] in exts and not any(e in p for e in excl):
                    files.append(p)
    files.sort()
    # per file: list of (lineno, normalized)
    norm = {}
    for p in files:
        ext = os.path.splitext(p)[1]
        lines = strip_comments(open(p, encoding='utf-8', errors='replace').read(), ext)
        seq = []
        for i, l in enumerate(lines, 1):
            if re.match(r'\s*#\[cfg\(test\)\]', l): break  # test modules sit at the end of a file
            if re.match(r'\s*(pub(\([a-z]+\))?\s+)?(mod|use)\b', l): continue  # module plumbing is not a clone
            n = norm_line(l, a.ids == 'mask')
            if n and len(n.split()) >= a.min_tokens:
                seq.append((i, n))
        norm[p] = seq
    W = a.window
    index = defaultdict(list)
    for p, seq in norm.items():
        for k in range(len(seq) - W + 1):
            key = hashlib.md5('\n'.join(x[1] for x in seq[k:k+W]).encode()).hexdigest()
            index[key].append((p, k))
    # pairs: (fileA, kA, fileB, kB) -> merge consecutive
    pairs = set()
    for key, occ in index.items():
        if len(occ) < 2: continue
        for i in range(len(occ)):
            for j in range(i+1, len(occ)):
                (pa, ka), (pb, kb) = occ[i], occ[j]
                if pa == pb and abs(ka-kb) < W: continue
                pairs.add((pa, ka, pb, kb))
    # merge: group by (pa,pb,kb-ka) then by runs of ka
    groups = defaultdict(list)
    for pa, ka, pb, kb in pairs:
        groups[(pa, pb, kb-ka)].append(ka)
    merged = []
    for (pa, pb, d), ks in groups.items():
        ks.sort()
        start = prev = ks[0]
        for k in ks[1:] + [None]:
            if k is not None and k <= prev + W:
                prev = k; continue
            # run start..prev (window starts) => lines seq[start] .. seq[prev+W-1]
            la = norm[pa][start][0]; lb_ = norm[pa][prev+W-1][0]
            ma = norm[pb][start+d][0]; mb = norm[pb][prev+W-1+d][0]
            merged.append((prev + W - start, pa, la, lb_, pb, ma, mb))
            if k is not None: start = prev = k
    # collapse periodic self-overlaps (a repeated 4-line block yields one
    # pair per shift): keep, per start line, only the smallest shift
    best = {}
    rest = []
    for m in merged:
        n, pa, la, lb_, pb, ma, mb = m
        if pa == pb and la <= mb and ma <= lb_:
            key = (pa, la)
            if key not in best or (best[key][5]-best[key][2]) > (ma-la):
                best[key] = m
        else:
            rest.append(m)
    spans = [(m[1], m[2], m[6]) for m in best.values()]
    def inside(m):
        return m[1] == m[4] and any(f == m[1] and lo <= m[2] and m[6] <= hi for f, lo, hi in spans)
    rest = [m for m in rest if not inside(m)]
    periodic = {(m[1], m[2]): m[5]-m[2] for m in best.values()}
    merged = rest + list(best.values())
    merged.sort(key=lambda x: -x[0])
    total = sum(m[0] for m in merged)
    print(f'files={len(files)} window={W} ids={a.ids} clone-pairs={len(merged)} normalised-lines-in-pairs={total}')
    for n, pa, la, lb_, pb, ma, mb in merged[:a.top]:
        tag = f'  [periodic block of {periodic[(pa,la)]} lines]' if (pa, la) in periodic else ''
        print(f'{n:3d} lines  {pa}:{la}-{lb_}  <->  {pb}:{ma}-{mb}{tag}')
    # per-file duplication share
    share = defaultdict(int)
    for n, pa, la, lb_, pb, ma, mb in merged:
        share[pa] += n; share[pb] += n
    print('\nper-file normalised lines involved in clones (top 25):')
    for p, n in sorted(share.items(), key=lambda x: -x[1])[:25]:
        print(f'{n:4d}/{len(norm[p]):4d}  {p}')

if __name__ == '__main__':
    main()
