#!/usr/bin/env python3
"""Module dependency graph for the `iter` crate (composability review, 2026-11-02).

Usage: python3 -I modgraph.py <repo-root> [--out <dir>]

Module = one .rs file; `foo/mod.rs` is the module `foo`; `main.rs` is `crate`.
Edges come from (a) `use crate::…` / `use super::…` / `use self::…` statements
(multi-line, braces expanded) and (b) qualified paths `crate::a::b`, `super::a`,
`self::a` anywhere in code. `pub use sub::{Item}` re-exports in a `mod.rs` are
followed so `crate::models::Task` points at `models/task`, not `models`.
Test code (`#[cfg(test)] mod … { … }` blocks and modules declared under
`#[cfg(test)]`) is excluded from the main graph; a second graph with tests
is built for the `fixtures` reachability check.

Outputs (in --out): graph.json, metrics.md (fan-in/out, SCCs, layer rules,
components, god modules, orphan pub items, config() readers), web.mmd, core.mmd.
"""
import json
import os
import re
import sys
from collections import defaultdict

ROOT = sys.argv[1]
OUT = sys.argv[sys.argv.index("--out") + 1] if "--out" in sys.argv else "."
SRC = os.path.join(ROOT, "src")

# --------------------------------------------------------------------------- files
files = {}  # module id -> path
for dp, _, fns in os.walk(SRC):
    for fn in fns:
        if not fn.endswith(".rs"):
            continue
        p = os.path.join(dp, fn)
        rel = os.path.relpath(p, SRC)[:-3]
        if rel == "main":
            mod = "crate"
        elif rel.endswith("/mod"):
            mod = rel[:-4]
        else:
            mod = rel
        files[mod] = p

src_text = {m: open(p, encoding="utf-8").read() for m, p in files.items()}
lines_of = {m: t.count("\n") + 1 for m, t in src_text.items()}


def strip_comments(t):
    t = re.sub(r"/\*.*?\*/", "", t, flags=re.S)
    return re.sub(r"//[^\n]*", "", t)


def split_tests(t):
    """Return (non_test, test) code. Handles `#[cfg(test)]` followed by `mod x {`."""
    non, test = [], []
    i = 0
    pat = re.compile(r"#\[cfg\(test\)\]\s*(?:pub(?:\([a-z]+\))?\s+)?mod\s+\w+\s*\{")
    while True:
        m = pat.search(t, i)
        if not m:
            non.append(t[i:])
            break
        non.append(t[i : m.start()])
        depth, j = 1, m.end()
        while j < len(t) and depth:
            c = t[j]
            if c == "{":
                depth += 1
            elif c == "}":
                depth -= 1
            j += 1
        test.append(t[m.end() : j])
        i = j
    return "".join(non), "".join(test)


code = {}  # module -> (non_test_code, test_code)
test_only_mods = set()
for m, t in src_text.items():
    t = strip_comments(t)
    non, test = split_tests(t)
    code[m] = (non, test)
    for mm in re.finditer(r"#\[cfg\(test\)\]\s*(?:pub(?:\([a-z]+\))?\s+)?mod\s+(\w+)\s*;", t):
        sub = mm.group(1)
        test_only_mods.add(sub if m == "crate" else f"{m}/{sub}")
# anything under a test-only module is test-only too
test_only_mods |= {m for m in files if any(m.startswith(x + "/") for x in test_only_mods)}

# --------------------------------------------------------------------------- re-exports
reexport = defaultdict(dict)  # module -> item name -> target module
USE_RE = re.compile(r"\buse\s+([^;]+);", re.S)


def expand_use(body):
    """Expand `a::{b, c::{d, e}}` into a list of path segment lists."""
    body = re.sub(r"\s+", " ", body).strip()
    out = []

    def rec(prefix, s):
        s = s.strip()
        if s.startswith("{") and s.endswith("}"):
            inner = s[1:-1]
            depth, start = 0, 0
            parts = []
            for i, c in enumerate(inner):
                if c == "{":
                    depth += 1
                elif c == "}":
                    depth -= 1
                elif c == "," and depth == 0:
                    parts.append(inner[start:i])
                    start = i + 1
            parts.append(inner[start:])
            for part in parts:
                if part.strip():
                    rec(prefix, part)
            return
        # strip `as alias`
        s = re.sub(r"\s+as\s+\w+$", "", s)
        if "{" in s:
            head, rest = s.split("{", 1)
            segs = [x for x in head.strip("::").strip().split("::") if x]
            rec(prefix + segs, "{" + rest)
            return
        segs = [x for x in s.split("::") if x]
        out.append(prefix + segs)

    rec([], body)
    return out


def resolve_prefix(from_mod, segs):
    """Turn crate/super/self relative path into an absolute module path list."""
    if not segs:
        return None
    base = [] if from_mod == "crate" else from_mod.split("/")
    i = 0
    if segs[0] == "crate":
        base, i = [], 1
    elif segs[0] == "self":
        i = 1
    elif segs[0] == "super":
        while i < len(segs) and segs[i] == "super":
            base = base[:-1]
            i += 1
    else:
        # a bare path whose head is a child module of `from_mod` (Rust 2018: `use child::Item`)
        child = segs[0] if from_mod == "crate" else f"{from_mod}/{segs[0]}"
        if child in files:
            return base + segs
        return None  # external crate or local item
    return base + segs[i:]


def longest_module(abs_segs):
    """Longest prefix of abs_segs that is a module; returns (module, rest)."""
    for k in range(len(abs_segs), 0, -1):
        cand = "/".join(abs_segs[:k])
        if cand in files:
            return cand, abs_segs[k:]
    return ("crate", abs_segs) if abs_segs else (None, [])


# first pass: record `pub use sub::Item` re-exports in every module
for m, (non, _) in code.items():
    for um in USE_RE.finditer(non):
        stmt = um.group(0)
        if not re.match(r"\s*pub", non[max(0, um.start() - 20) : um.start()].split("\n")[-1] + stmt):
            # check the token right before `use`
            pre = non[max(0, um.start() - 40) : um.start()]
            if not re.search(r"pub(\([a-z]+\))?\s*$", pre):
                continue
        for segs in expand_use(um.group(1)):
            abs_segs = resolve_prefix(m, segs)
            if abs_segs is None:
                # `pub use sub::Item` with a bare child module name
                if segs and (f"{m}/{segs[0]}" in files or (m == "crate" and segs[0] in files)):
                    abs_segs = ([] if m == "crate" else m.split("/")) + segs
                else:
                    continue
            target, rest = longest_module(abs_segs)
            if target and target != m:
                item = rest[-1] if rest else "*"
                reexport[m][item] = target
                if item == "*":
                    reexport[m].setdefault("*", target)
                    reexport[m].setdefault("__globs__", [])
                    reexport[m]["__globs__"].append(target)


def resolve_item(target, rest):
    """Follow a re-export chain from module `target` for item path `rest`."""
    seen = set()
    while rest and target in reexport and target not in seen:
        seen.add(target)
        item = rest[0]
        tbl = reexport[target]
        if item in tbl:
            target = tbl[item]
            rest = rest[1:]
            # the target may itself be a module and the rest a submodule
            if rest and f"{target}/{rest[0]}" in files:
                target, rest = f"{target}/{rest[0]}", rest[1:]
            continue
        globs = tbl.get("__globs__", [])
        hit = None
        for g in globs:
            if item in pub_items.get(g, set()):
                hit = g
                break
        if hit:
            target, rest = hit, rest[1:]
            continue
        break
    return target


# public items per module (used for glob re-export resolution and orphans)
PUB_RE = re.compile(
    r"\bpub(?:\((?:crate|super|self)\))?\s+(?:async\s+)?(?:unsafe\s+)?(fn|struct|enum|const|static|trait|type|mod|macro_rules!)\s+([A-Za-z_][A-Za-z0-9_]*)"
)
pub_items = defaultdict(set)
pub_item_lines = {}
for m, (non, _) in code.items():
    for pm in PUB_RE.finditer(non):
        kind, name = pm.group(1), pm.group(2)
        if kind == "mod":
            continue
        pub_items[m].add(name)
        # line number in the original file
        orig = src_text[m]
        om = re.search(r"\bpub(?:\([a-z]+\))?\s+(?:async\s+)?(?:unsafe\s+)?" + kind + r"\s+" + name + r"\b", orig)
        pub_item_lines[(m, name)] = orig[: om.start()].count("\n") + 1 if om else 0
# `#[component]` functions are pub via macro even if written `fn`
COMP_RE = re.compile(r"#\[component[^\]]*\]\s*(?:pub(?:\([a-z]+\))?\s+)?(?:async\s+)?fn\s+(\w+)")
components = {}  # name -> module
for m, (non, _) in code.items():
    for cm in COMP_RE.finditer(non):
        components[cm.group(1)] = m

# --------------------------------------------------------------------------- edges
QUAL_RE = re.compile(r"\b(crate|super|self)(::\w+)+")


def edges_from(m, text):
    """Return dict target_module -> set of imported item names / qualified paths."""
    out = defaultdict(set)
    body = text
    for um in USE_RE.finditer(body):
        for segs in expand_use(um.group(1)):
            abs_segs = resolve_prefix(m, segs)
            if abs_segs is None:
                continue
            target, rest = longest_module(abs_segs)
            if target is None:
                continue
            target = resolve_item(target, rest)
            if target != m:
                out[target].add("::".join(rest) or "*")
    body_wo_use = USE_RE.sub("", body)
    for qm in QUAL_RE.finditer(body_wo_use):
        segs = qm.group(0).split("::")
        abs_segs = resolve_prefix(m, segs)
        if abs_segs is None:
            continue
        target, rest = longest_module(abs_segs)
        if target is None:
            continue
        target = resolve_item(target, rest)
        if target != m:
            out[target].add("q:" + "::".join(rest))
    return out


graph = {}  # non-test
graph_all = {}  # incl. test code
for m, (non, test) in code.items():
    if m in test_only_mods:
        graph[m] = {}
    else:
        graph[m] = {k: sorted(v) for k, v in edges_from(m, non).items()}
    both = edges_from(m, non + "\n" + test)
    graph_all[m] = {k: sorted(v) for k, v in both.items()}

fan_out = {m: len(g) for m, g in graph.items()}
fan_in = defaultdict(int)
dependents = defaultdict(list)
for m, g in graph.items():
    for t in g:
        fan_in[t] += 1
        dependents[t].append(m)
for m in files:
    fan_in.setdefault(m, 0)


# --------------------------------------------------------------------------- SCC (Tarjan)
def tarjan(g):
    index, low, on, st, res = {}, {}, set(), [], []
    counter = [0]

    def visit(v):
        index[v] = low[v] = counter[0]
        counter[0] += 1
        st.append(v)
        on.add(v)
        for w in g.get(v, {}):
            if w not in index:
                visit(w)
                low[v] = min(low[v], low[w])
            elif w in on:
                low[v] = min(low[v], index[w])
        if low[v] == index[v]:
            comp = []
            while True:
                w = st.pop()
                on.discard(w)
                comp.append(w)
                if w == v:
                    break
            res.append(sorted(comp))

    sys.setrecursionlimit(10000)
    for v in sorted(g):
        if v not in index:
            visit(v)
    return [c for c in res if len(c) > 1]


sccs = tarjan(graph)
sccs_all = tarjan(graph_all)


def scc_edges(comp):
    s = set(comp)
    return [(a, b, graph[a][b]) for a in comp for b in graph[a] if b in s]


# --------------------------------------------------------------------------- layer rules
def is_ui(m):
    return m.startswith("web/ui")


def is_web_page(m):
    return m.startswith("web") and not is_ui(m)


violations = []
for m, g in graph.items():
    for t in g:
        if is_ui(m) and (is_web_page(t) or t.startswith("db") or t.startswith("commands")):
            if t == "web/assets":
                continue  # the documented exception (asset bundling)
            violations.append(("ui->page/db/commands", m, t, g[t]))
        if (m.startswith("models") or m.startswith("reporting")) and (t.startswith("db") or t.startswith("web")):
            violations.append(("models/reporting->db/web", m, t, g[t]))
        if m.startswith("web") and t.startswith("commands"):
            violations.append(("web->commands", m, t, g[t]))

# --------------------------------------------------------------------------- component call sites
comp_calls = {}
for name, defmod in components.items():
    pat = re.compile(r"(?<![\w:])" + re.escape(name) + r"\s*\(")
    sites = defaultdict(int)
    for m, (non, _) in code.items():
        if m in test_only_mods:
            continue
        # only modules that import the component by name (or glob) from its
        # defining module, or the defining module itself: avoids homonyms
        # (`screen`, `cell`, `status` exist as plain fns in many modules)
        imported = graph.get(m, {}).get(defmod, [])
        if m != defmod and not any(x == name or x == "*" or x == "q:" + name for x in imported):
            continue
        n = len(pat.findall(non))
        if m == defmod:
            # drop the definition itself
            n -= len(re.findall(r"fn\s+" + re.escape(name) + r"\s*\(", non))
        if n > 0:
            sites[m] += n
    comp_calls[name] = (defmod, dict(sites))

# --------------------------------------------------------------------------- orphan pub items
orphans = []
for m, items in pub_items.items():
    if m in test_only_mods:
        continue
    for name in sorted(items):
        pat = re.compile(r"\b" + re.escape(name) + r"\b")
        used_elsewhere = any(
            pat.search(code[o][0]) for o in code if o != m and o not in test_only_mods
        )
        if used_elsewhere:
            continue
        used_in_tests = any(pat.search(code[o][1]) for o in code) or any(
            pat.search(code[o][0]) for o in test_only_mods if o in code
        )
        own_uses = len(pat.findall(code[m][0])) - 1
        orphans.append((m, name, pub_item_lines.get((m, name), 0), own_uses, used_in_tests))

# --------------------------------------------------------------------------- config() readers
config_readers = {}
for m, (non, _) in code.items():
    if m in test_only_mods or m == "config":
        continue
    n = len(re.findall(r"(?<![\w:])config\(\)", non))
    if n:
        config_readers[m] = n

# --------------------------------------------------------------------------- fixtures reachability
def reachable_to(target, g):
    rev = defaultdict(set)
    for a, ts in g.items():
        for b in ts:
            rev[b].add(a)
    seen, stack = set(), [target]
    while stack:
        v = stack.pop()
        for w in rev[v]:
            if w not in seen:
                seen.add(w)
                stack.append(w)
    return seen


fixtures_users_nontest = sorted(m for m, g in graph.items() if "fixtures" in g)
fixtures_users_all = sorted(m for m, g in graph_all.items() if "fixtures" in g)

# --------------------------------------------------------------------------- output
os.makedirs(OUT, exist_ok=True)
with open(os.path.join(OUT, "graph.json"), "w") as f:
    json.dump(
        {
            "graph": graph,
            "graph_with_tests": graph_all,
            "lines": lines_of,
            "test_only": sorted(test_only_mods),
            "reexports": {k: {i: t for i, t in v.items() if i != "__globs__"} for k, v in reexport.items()},
        },
        f,
        indent=1,
        sort_keys=True,
    )

md = []
P = md.append
P("# Module graph metrics (generated by modgraph.py)\n")
P(f"Modules: {len(files)} (test-only: {len(test_only_mods)}); edges (non-test): {sum(fan_out.values())}\n")

P("## Top 20 by fan-out (distinct modules imported, non-test code)\n")
P("| module | fan-out | fan-in | lines |\n|---|---|---|---|")
for m in sorted(files, key=lambda x: (-fan_out[x], x))[:20]:
    P(f"| `{m}` | {fan_out[m]} | {fan_in[m]} | {lines_of[m]} |")
P("\n## Top 20 by fan-in\n")
P("| module | fan-in | fan-out | lines |\n|---|---|---|---|")
for m in sorted(files, key=lambda x: (-fan_in[x], x))[:20]:
    P(f"| `{m}` | {fan_in[m]} | {fan_out[m]} | {lines_of[m]} |")

P("\n## Strongly connected components with > 1 node (non-test code)\n")
for i, comp in enumerate(sccs, 1):
    P(f"### SCC {i}: {len(comp)} modules\n")
    P(", ".join(f"`{c}`" for c in comp) + "\n")
    for a, b, items in scc_edges(comp):
        P(f"- `{a}` -> `{b}`: {', '.join(items[:6])}{' …' if len(items) > 6 else ''}")
    P("")
P("### SCCs including test code\n")
for comp in sccs_all:
    P("- " + ", ".join(f"`{c}`" for c in comp))

P("\n## Layer rule violations\n")
if not violations:
    P("none")
for rule, a, b, items in violations:
    P(f"- [{rule}] `{a}` -> `{b}`: {', '.join(items[:6])}")

P("\n## God modules (fan-out > 12 or lines > 500, non-test)\n")
P("| module | fan-out | lines |\n|---|---|---|")
for m in sorted(files, key=lambda x: (-fan_out[x], -lines_of[x])):
    if m in test_only_mods:
        continue
    if fan_out[m] > 12 or lines_of[m] > 500:
        P(f"| `{m}` | {fan_out[m]} | {lines_of[m]} |")

P("\n## `#[component]` call sites (non-test code)\n")
P("| component | defined in | call sites | distinct callers |\n|---|---|---|---|")
for name in sorted(components, key=lambda n: (sum(comp_calls[n][1].values()), n)):
    defmod, sites = comp_calls[name]
    total = sum(sites.values())
    P(f"| `{name}` | `{defmod}` | {total} | {', '.join(f'`{k}`×{v}' for k, v in sorted(sites.items()))} |")

P("\n## Orphan `pub` items (name not referenced in any other non-test module)\n")
P("| module:line | item | uses in own file | used in tests |\n|---|---|---|---|")
for m, name, ln, own, tests in sorted(orphans):
    P(f"| `{m}:{ln}` | `{name}` | {own} | {'yes' if tests else 'no'} |")

P("\n## `config()` readers (call count per module)\n")
P("| module | calls |\n|---|---|")
for m, n in sorted(config_readers.items(), key=lambda kv: (-kv[1], kv[0])):
    P(f"| `{m}` | {n} |")
P(f"\nTotal modules reading `config()`: {len(config_readers)}, calls: {sum(config_readers.values())}")

P("\n## fixtures reachability\n")
P(f"- non-test modules importing `fixtures`: {fixtures_users_nontest or 'none'}")
P(f"- modules importing `fixtures` incl. test code: {len(fixtures_users_all)}")
P(f"- test-only modules: {sorted(test_only_mods)}")

P("\n## Dependents per module (fan-in detail, non-test)\n")
for m in sorted(files, key=lambda x: (-fan_in[x], x)):
    if fan_in[m]:
        P(f"- `{m}` ({fan_in[m]}): {', '.join(sorted(dependents[m]))}")

with open(os.path.join(OUT, "metrics.md"), "w") as f:
    f.write("\n".join(md) + "\n")


# --------------------------------------------------------------------------- mermaid
def mid(m):
    return re.sub(r"[^A-Za-z0-9_]", "_", m)


CLUSTERS = [
    ("boards", ("agenda", "agenda_cards", "agenda_day", "agenda_drop", "agenda_pick", "board_cards",
                "board_header", "board_info", "board_page", "boards", "drop", "pick", "quadrants",
                "matrix", "board_edit")),
    ("org", ("org", "org_report", "org_share", "org_edit", "platform_pick")),
    ("project", ("project", "project_new", "project_edit", "project_rows", "project_options",
                 "folders", "folder_picker")),
    ("task", ("task", "task_agent", "task_runs", "task_new", "task_edit", "task_fields", "task_panel",
              "task_rows", "task_query", "task_notes", "task_sessions", "task_wiki", "session_lines")),
    ("agents_instructions", ("agents", "instructions")),
    ("edit_forms_settings", ("edit", "edit_page", "forms", "settings", "settings_fields", "settings_panel")),
    ("notes", ("daily_notes", "notes")),
    ("shell", ("home", "layout", "footer", "errors", "guard", "blocking", "assets", "search",
               "search_index", "crumbs", "sections", "url", "load", "text")),
]


def web_cluster(m):
    if m == "web":
        return "shell"
    if is_ui(m):
        return "ui"
    if not m.startswith("web/"):
        return m.split("/")[0]
    leaf = m[4:].split("/")[0]
    for name, members in CLUSTERS:
        if leaf in members:
            return name
    return "other"


def web_mermaid():
    """Web layer, cluster level: edge label = number of module-level edges."""
    edges = defaultdict(int)
    for m, g in graph.items():
        if not m.startswith("web") or m in test_only_mods:
            continue
        a = web_cluster(m)
        for t in g:
            b = web_cluster(t)
            if a == b:
                continue
            edges[(a, b)] += 1
    out = ["flowchart LR"]
    out.append("  subgraph WEB[src/web by family]")
    for name, _ in CLUSTERS:
        out.append(f"    {mid(name)}[{name}]")
    out.append("  end")
    out.append("  ui[web/ui/* collapsed]")
    core = sorted({b for _, b in edges if b not in dict(CLUSTERS) and b != "ui"})
    out.append("  subgraph CORE[core crate]")
    for c in core:
        out.append(f"    {mid(c)}[{c}]")
    out.append("  end")
    for (a, b), n in sorted(edges.items()):
        style = "-.->" if (b, a) in edges else "-->"
        out.append(f"  {mid(a)} {style}|{n}| {mid(b)}")
    return "\n".join(out)


def scc_mermaid():
    """Module-level detail of every web SCC: only intra-SCC edges, labelled with the items."""
    out = ["flowchart LR"]
    for i, comp in enumerate(sccs, 1):
        if not comp[0].startswith("web"):
            continue
        out.append(f"  subgraph SCC{i}[cycle {i}: {len(comp)} modules]")
        for c in comp:
            out.append(f"    {mid(c)}[{c[4:]}]")
        out.append("  end")
        for a, b, items in scc_edges(comp):
            label = ", ".join(x.replace("q:", "") for x in items[:3]) + (" …" if len(items) > 3 else "")
            out.append(f"  {mid(a)} -->|{label}| {mid(b)}")
    return "\n".join(out)


def core_scc_mermaid():
    comp = next((c for c in sccs if "config" in c), None)
    out = ["flowchart LR"]
    if not comp:
        return "\n".join(out)
    fam = lambda m: m.split("/")[0]
    fams = sorted({fam(c) for c in comp})
    for f in fams:
        out.append(f"  {mid(f)}[{f}]")
    seen = set()
    for a, b, items in scc_edges(comp):
        fa, fb = fam(a), fam(b)
        if fa == fb or (fa, fb) in seen:
            continue
        seen.add((fa, fb))
        label = ", ".join(x.replace("q:", "") for x in items[:3])
        out.append(f"  {mid(fa)} -->|{label}| {mid(fb)}")
    return "\n".join(out)


def core_mermaid():
    """Core layer: directories collapsed, web collapsed to one node."""

    def node(m):
        if m.startswith("web"):
            return "web"
        if m.startswith("args"):
            return "args"
        return m.split("/")[0] if "/" in m else m

    edges = defaultdict(set)
    for m, g in graph.items():
        if m in test_only_mods:
            continue
        a = node(m)
        for t in g:
            b = node(t)
            if a == b or t in test_only_mods:
                continue
            edges[(a, b)].add(m)
    out = ["flowchart LR"]
    nodes = sorted({a for a, _ in edges} | {b for _, b in edges})
    for n in nodes:
        out.append(f"  {mid(n)}[{n}]")
    skip = {"error", "config"}  # universal leaves, listed in text
    out.append("  %% edges into `error` and `config` omitted (every module); see fan-in table")
    for (a, b), srcs in sorted(edges.items()):
        if b in skip:
            continue
        style = "-.->" if (b, a) in edges else "-->"
        out.append(f"  {mid(a)} {style} {mid(b)}")
    return "\n".join(out)


with open(os.path.join(OUT, "web.mmd"), "w") as f:
    f.write(web_mermaid() + "\n")
with open(os.path.join(OUT, "core.mmd"), "w") as f:
    f.write(core_mermaid() + "\n")
with open(os.path.join(OUT, "web_sccs.mmd"), "w") as f:
    f.write(scc_mermaid() + "\n")
with open(os.path.join(OUT, "core_scc.mmd"), "w") as f:
    f.write(core_scc_mermaid() + "\n")

print(f"modules={len(files)} edges={sum(fan_out.values())} sccs={len(sccs)} violations={len(violations)} components={len(components)} orphans={len(orphans)}")
for comp in sccs:
    print("SCC:", comp)
for v in violations:
    print("VIOLATION:", v[:3])
