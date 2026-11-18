#!/usr/bin/env python3
"""Approximate per-crate text size of a Mach-O Rust binary from `nm -n` (address deltas; legacy mangling)."""
import re, subprocess, sys, collections
binp = sys.argv[1]
out = subprocess.run(["nm", "-n", "-U", "--defined-only", binp], capture_output=True, text=True).stdout
def v0_crate(name):
    """Crate root of a v0-mangled symbol (minimal path parser: N<ns>, M/X impl paths, I generics, s<b62>_ disambiguators, C root)."""
    if not name.startswith("__R") and not name.startswith("_R"): return None
    i = name.index("R") + 1
    for _ in range(64):
        if i >= len(name): return None
        c = name[i]
        if c == "s":
            j = i + 1
            while j < len(name) and name[j] != "_": j += 1
            i = j + 1
        elif c == "C":
            i += 1
            if i < len(name) and name[i] == "s":
                j = i + 1
                while j < len(name) and name[j] != "_": j += 1
                i = j + 1
            m = re.match(r"(\d+)(u?)", name[i:])
            if not m: return None
            ln = int(m.group(1)); i += m.end()
            return name[i:i+ln]
        elif c == "N": i += 2
        elif c in "MXI": i += 1
        else: return None
    return None

syms = []
for line in out.splitlines():
    parts = line.split()
    if len(parts) < 3: continue
    addr, typ, name = int(parts[0], 16), parts[1], parts[2]
    if typ.lower() not in ("t",): continue
    syms.append((addr, name))
syms.sort()
sizes = collections.Counter(); n = collections.Counter()
for (a, name), (b, _) in zip(syms, syms[1:]):
    sz = b - a
    m = re.match(r"_?_ZN(\d+)([A-Za-z_][A-Za-z0-9_]*)", name)
    v0 = v0_crate(name)
    if m:
        ln = int(m.group(1)); crate = m.group(2)[:ln]
        crate = re.sub(r"^_\$LT\$.*", "<trait impl>", crate)
    elif v0:
        crate = v0
    elif name.startswith("_sqlite3") or name.startswith("_sqlite"):
        crate = "sqlite3 (C)"
    elif name.startswith("_ring_") or "ring_core" in name or name.startswith("_GFp") or name.startswith("_x25519") or name.startswith("_fiat") or name.startswith("_p256") or name.startswith("_p384") or name.startswith("_aes") or name.startswith("_gcm") or name.startswith("_poly1305") or name.startswith("_chacha") or name.startswith("_ecp") or name.startswith("_bn_") or name.startswith("_LIMBS"):
        crate = "ring (C/asm)"
    else:
        crate = "other/C/unmangled"
    sizes[crate] += sz; n[crate] += 1
tot = sum(sizes.values())
print(f"text total ~{tot/1048576:.2f} MB over {len(syms)} symbols")
for c, s in sizes.most_common(int(sys.argv[2]) if len(sys.argv) > 2 else 35):
    print(f"{s/1048576:7.2f} MB {100*s/tot:5.1f}% {n[c]:6d} syms  {c}")
