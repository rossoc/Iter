#!/usr/bin/env python3
"""Simulate removing dependency edges from the resolved graph (cargo metadata) and count crates that disappear.
Usage: prune_sim.py [--no-default-features] "<from>:<to>" ...   (crate names; '*' as <from> = any)
Counts normal+build dep edges (what a `cargo build` compiles), not dev-deps."""
import json, subprocess, sys
args = [a for a in sys.argv[1:] if a.startswith("--")]
cuts = [a.split(":") for a in sys.argv[1:] if not a.startswith("--")]
meta = json.loads(subprocess.run(["cargo", "metadata", "--format-version", "1"] + args, capture_output=True, text=True).stdout)
pk = {p["id"]: p for p in meta["packages"]}
root = meta["resolve"]["root"]
nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
def edges(nid, cutset):
    for d in nodes[nid]["deps"]:
        kinds = {k["kind"] for k in d["dep_kinds"]}
        if kinds <= {"dev"}: continue
        if (pk[nid]["name"], pk[d["pkg"]]["name"]) in cutset or ("*", pk[d["pkg"]]["name"]) in cutset: continue
        yield d["pkg"]
def reach(cutset):
    seen = {root}; stack = [root]
    while stack:
        n = stack.pop()
        for m in edges(n, cutset):
            if m not in seen: seen.add(m); stack.append(m)
    return seen
base = reach(set())
print(f"baseline: {len(base)} packages (normal+build, incl. root)")
cutset = {tuple(c) for c in cuts}
after = reach(cutset)
gone = sorted(pk[i]["name"] + " " + pk[i]["version"] for i in base - after)
print(f"after cutting {cuts}: {len(after)} packages, -{len(gone)}:")
print("  " + ", ".join(gone))
