//! The architecture test (task 298, R5): the layering of sketch `architecture`, read from the
//! source text of `src/**/*.rs`. It runs no iter code and touches no tmux, config or database.
//!
//! Comments and `#[cfg(test)]` items are stripped before anything is counted, as `deps.py` does,
//! so a rule is about production code only. Each rule is its own test; while isolation is open
//! the ones not true yet carry `#[ignore = "isolation: round I3"]` (step 6, the `cli` module, makes
//! them true) and whoever makes a rule true removes its `ignore`; the others guard what holds today. Run one with
//! `cargo test --test arch -- --ignored <name>`: it only reads files.

use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// The core modules and their rank: a module imports only modules of a lower rank (sketch
/// `architecture`, section 1). `agent` is the top of what the others may not reach.
const RANKS: [(&str, u8); 26] = [
    ("error", 0),
    ("text", 1),
    ("files", 1),
    ("process", 2),
    ("models", 2),
    ("config", 3),
    ("db", 3),
    ("git", 3),
    ("github", 3),
    ("secret", 3),
    ("app", 4),
    ("tmux", 4),
    ("reporting", 4),
    ("notes", 4),
    ("editor", 4),
    ("sync", 4),
    ("scaffold", 4),
    ("front", 4),
    ("platform", 5),
    ("locate", 5),
    ("workspace", 6),
    ("push", 6),
    ("wiki", 6),
    ("agent", 7),
    ("note_ref", 8),
    // test support, outside the ranks: nothing imports it in production
    ("fixtures", 255),
];

/// The interface: what a person drives. Before step 6 it is three modules, after it one.
const INTERFACE: [&str; 5] = ["web", "commands", "args", "completion", "cli"];

/// Where the `rusqlite` crate and SQL text may appear in the core.
const SQL_HOMES: [&str; 3] = ["db", "models", "error"];

const SQL: &str = r"SELECT\b.*\bFROM\b|INSERT INTO |UPDATE \w+ SET |DELETE FROM |CREATE (TABLE|INDEX) |ALTER TABLE |PRAGMA ";

struct Scan {
    /// Code with comments and `#[cfg(test)]` items gone and string literals emptied.
    blank: String,
    /// The same with string literals as written (for `cfg(feature = "web")`).
    full: String,
    /// Every string literal's text.
    strings: Vec<String>,
}

/// One pass over Rust source: strips comments and `#[cfg(test)]` items, collects strings.
fn scan(src: &str) -> Scan {
    let c: Vec<char> = src.chars().collect();
    let (mut blank, mut full, mut strings) = (String::new(), String::new(), Vec::new());
    // None: emitting; Some(0): after `#[cfg(test)]`, before its `{` or `;`; Some(n): n braces deep
    let mut skip: Option<u32> = None;
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        let next = c.get(i + 1).copied();
        // comments
        if ch == '/' && next == Some('/') {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if ch == '/' && next == Some('*') {
            let mut depth = 0;
            while i < c.len() {
                if c[i] == '/' && c.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if c[i] == '*' && c.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            continue;
        }
        // string literals, raw or not
        let raw = ch == 'r' && (i == 0 || !(c[i - 1].is_alphanumeric() || c[i - 1] == '_')) && {
            let hashes = c[i + 1..].iter().take_while(|&&h| h == '#').count();
            c.get(i + 1 + hashes) == Some(&'"')
        };
        if ch == '"' || raw {
            let (hashes, start) = if raw {
                let h = c[i + 1..].iter().take_while(|&&h| h == '#').count();
                (h, i + 2 + h)
            } else {
                (0, i + 1)
            };
            let mut j = start;
            while j < c.len() {
                if !raw && c[j] == '\\' {
                    j += 2;
                    continue;
                }
                if c[j] == '"' && c[j + 1..].iter().take_while(|&&h| h == '#').count() >= hashes {
                    break;
                }
                j += 1;
            }
            let end = j.min(c.len());
            let text: String = c[start..end].iter().collect();
            let literal: String = c[i..(end + 1 + hashes).min(c.len())].iter().collect();
            if skip.is_none() {
                full.push_str(&literal);
                blank.push_str("\"\"");
                strings.push(text);
            }
            i = end + 1 + hashes;
            continue;
        }
        // a char literal (a lifetime has no closing quote)
        if ch == '\'' {
            let len = match (next, c.get(i + 2)) {
                (Some('\\'), _) => c[i + 2..].iter().position(|&q| q == '\'').map(|p| p + 3),
                (Some(_), Some('\'')) => Some(3),
                _ => None,
            };
            if let Some(len) = len {
                if skip.is_none() {
                    full.extend(&c[i..i + len]);
                    blank.push_str("' '");
                }
                i += len;
                continue;
            }
        }
        i += 1;
        match &mut skip {
            Some(0) => match ch {
                '{' => skip = Some(1),
                ';' => skip = None,
                _ => {}
            },
            Some(depth) => match ch {
                '{' => *depth += 1,
                '}' => {
                    *depth -= 1;
                    if *depth == 0 {
                        skip = None;
                    }
                }
                _ => {}
            },
            None => {
                full.push(ch);
                blank.push(ch);
                if ch == ']' && blank.ends_with("#[cfg(test)]") {
                    let keep = blank.len() - "#[cfg(test)]".len();
                    blank.truncate(keep);
                    full.truncate(full.len() - "#[cfg(test)]".len());
                    skip = Some(0);
                }
            }
        }
    }
    Scan {
        blank,
        full,
        strings,
    }
}

struct Source {
    /// Path under `src/`, like `web/flash.rs`.
    file: String,
    /// The module path of the file: `web/flash.rs` is `[web, flash]`, `web/mod.rs` is `[web]`.
    path: Vec<String>,
    scan: Scan,
}

impl Source {
    /// The top-level module the file belongs to (`None`: `main.rs`).
    fn module(&self) -> Option<&str> {
        self.path.first().map(String::as_str)
    }
}

fn is_test_file(name: &str) -> bool {
    name == "tests.rs"
        || name.ends_with("_tests.rs")
        || name == "testutil.rs"
        || name == "fixtures.rs"
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("a readable folder").flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            // a graph cache some tool left under src/, not code
            if name != "graphify-out" {
                walk(&path, out);
            }
        } else if name.ends_with(".rs") && !is_test_file(&name) {
            out.push(path);
        }
    }
}

/// Every production source file of `src/`.
fn sources() -> Vec<Source> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    walk(&root, &mut files);
    files.sort();
    files
        .into_iter()
        .map(|path| {
            let rel = path.strip_prefix(&root).expect("under src");
            let mut parts: Vec<String> = rel
                .with_extension("")
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            if parts.last().is_some_and(|p| p == "mod") {
                parts.pop();
            }
            if parts == ["main"] {
                parts.clear();
            }
            Source {
                file: rel.display().to_string(),
                path: parts,
                scan: scan(&fs::read_to_string(&path).expect("a readable file")),
            }
        })
        .collect()
}

/// The top-level modules of `src/`: every folder and file but `main.rs`.
fn modules(sources: &[Source]) -> BTreeSet<String> {
    sources
        .iter()
        .filter_map(|s| s.module().map(String::from))
        .collect()
}

/// `module -> (imported module -> files that name it)`, production code only.
fn imports(sources: &[Source]) -> BTreeMap<String, BTreeMap<String, BTreeSet<String>>> {
    let known = modules(sources);
    let single = Regex::new(r"\bcrate::(\w+)").expect("regex");
    let group = Regex::new(r"\bcrate::\{([^}]*)\}").expect("regex");
    let up = Regex::new(r"\b((?:super::)+)(\w+)").expect("regex");
    let mut graph: BTreeMap<String, BTreeMap<String, BTreeSet<String>>> = BTreeMap::new();
    for source in sources {
        let Some(from) = source.module() else {
            continue;
        };
        let code = &source.scan.blank;
        let mut targets: Vec<String> = single
            .captures_iter(code)
            .map(|m| m[1].to_string())
            .collect();
        for g in group.captures_iter(code) {
            // `use crate::{a, b::c, d::{e, f}}`: the first name of each entry at depth 0
            let mut depth = 0;
            let mut entry = String::new();
            for ch in g[1].chars().chain([',']) {
                match ch {
                    '{' => depth += 1,
                    '}' => depth -= 1,
                    ',' if depth == 0 => {
                        let name: String = entry
                            .trim()
                            .chars()
                            .take_while(|c| c.is_alphanumeric() || *c == '_')
                            .collect();
                        targets.push(name);
                        entry.clear();
                        continue;
                    }
                    _ => {}
                }
                entry.push(ch);
            }
        }
        for m in up.captures_iter(code) {
            // `super::` climbs from the file's own module path; reaching the root names a top module
            let climb = m[1].matches("super::").count();
            if source.path.len() <= climb {
                targets.push(m[2].to_string());
            }
        }
        for to in targets {
            if known.contains(&to) && to != from {
                graph
                    .entry(from.to_string())
                    .or_default()
                    .entry(to)
                    .or_default()
                    .insert(source.file.clone());
            }
        }
    }
    graph
}

fn is_interface(module: &str) -> bool {
    INTERFACE.contains(&module)
}

fn rank(module: &str) -> Option<u8> {
    RANKS.iter().find(|(m, _)| *m == module).map(|(_, r)| *r)
}

/// Every edge as a line a failure message can show.
fn show(
    graph: &BTreeMap<String, BTreeMap<String, BTreeSet<String>>>,
    from: &str,
    to: &str,
) -> String {
    let files = &graph[from][to];
    let files: Vec<_> = files.iter().take(3).cloned().collect();
    format!("{from} -> {to} ({})", files.join(", "))
}

#[test]
#[ignore = "isolation: round I3"]
fn no_cycle_between_top_level_modules() {
    let graph = imports(&sources());
    let nodes: BTreeSet<&String> = graph
        .keys()
        .chain(graph.values().flat_map(BTreeMap::keys))
        .collect();
    // reach[a] = what a imports, directly or not
    let mut reach: BTreeMap<&String, BTreeSet<&String>> = nodes
        .iter()
        .map(|&n| {
            let direct = graph.get(n).map(|m| m.keys().collect()).unwrap_or_default();
            (n, direct)
        })
        .collect();
    loop {
        let mut grew = false;
        for &n in &nodes {
            let more: Vec<&String> = reach[n]
                .iter()
                .flat_map(|m| reach[*m].iter().copied())
                .collect();
            for m in more {
                grew |= reach.get_mut(n).expect("node").insert(m);
            }
        }
        if !grew {
            break;
        }
    }
    let cyclic: Vec<&&String> = nodes.iter().filter(|n| reach[**n].contains(**n)).collect();
    assert!(
        cyclic.is_empty(),
        "{} modules are on an import cycle: {:?}",
        cyclic.len(),
        cyclic
    );
}

#[test]
#[ignore = "isolation: round I3"]
fn every_module_is_in_the_design_and_imports_only_lower_ranks() {
    let sources = sources();
    let graph = imports(&sources);
    let mut problems = Vec::new();
    for module in modules(&sources) {
        if is_interface(&module) {
            continue;
        }
        let Some(own) = rank(&module) else {
            problems.push(format!("`{module}` is not a module of the design"));
            continue;
        };
        for to in graph.get(&module).into_iter().flat_map(BTreeMap::keys) {
            match rank(to) {
                Some(r) if r < own => {}
                Some(_) => problems.push(format!("not downward: {}", show(&graph, &module, to))),
                None => problems.push(format!(
                    "core into the interface: {}",
                    show(&graph, &module, to)
                )),
            }
        }
    }
    assert!(
        problems.is_empty(),
        "{}:\n{}",
        problems.len(),
        problems.join("\n")
    );
}

#[test]
#[ignore = "isolation: round I3"]
fn the_core_imports_nothing_from_the_interface() {
    let graph = imports(&sources());
    let bad: Vec<String> = graph
        .iter()
        .filter(|(from, _)| !is_interface(from))
        .flat_map(|(from, to)| {
            to.keys()
                .filter(|t| is_interface(t))
                .map(|t| show(&graph, from, t))
        })
        .collect();
    assert!(bad.is_empty(), "{}:\n{}", bad.len(), bad.join("\n"));
}

#[test]
fn the_interface_front_ends_do_not_import_each_other() {
    let graph = imports(&sources());
    let bad: Vec<String> = graph
        .iter()
        .filter(|(from, _)| is_interface(from))
        .flat_map(|(from, to)| {
            // `web` is one front end; commands, args, completion and cli are the other
            to.keys()
                .filter(move |t| is_interface(t) && (*t == "web") != (from == "web"))
                .map(|t| show(&graph, from, t))
        })
        .collect();
    assert!(bad.is_empty(), "{}:\n{}", bad.len(), bad.join("\n"));
}

#[test]
fn the_interface_holds_no_sql_and_no_rusqlite() {
    let sql = Regex::new(SQL).expect("regex");
    let rows = Regex::new(r"::COLUMNS|::LIST_TAIL|\bfrom_row\(").expect("regex");
    let mut bad = Vec::new();
    for s in sources()
        .iter()
        .filter(|s| s.module().is_some_and(is_interface))
    {
        bad.extend(s.scan.strings.iter().filter(|t| sql.is_match(t)).map(|t| {
            format!(
                "{}: SQL text {:?}",
                s.file,
                t.chars().take(40).collect::<String>()
            )
        }));
        if rows.is_match(&s.scan.blank) {
            bad.push(format!("{}: names a table's layout", s.file));
        }
        if s.scan.blank.contains("rusqlite") {
            bad.push(format!("{}: names rusqlite", s.file));
        }
    }
    assert!(bad.is_empty(), "{}:\n{}", bad.len(), bad.join("\n"));
}

#[test]
fn sql_and_rusqlite_stay_in_the_storage_modules() {
    let sql = Regex::new(SQL).expect("regex");
    let mut bad = Vec::new();
    for s in sources() {
        let Some(module) = s.module() else { continue };
        if is_interface(module) {
            continue;
        }
        if !SQL_HOMES.contains(&module) && s.scan.blank.contains("rusqlite") {
            bad.push(format!("{}: names rusqlite", s.file));
        }
        if module != "db" && s.scan.strings.iter().any(|t| sql.is_match(t)) {
            bad.push(format!("{}: has SQL text outside `db`", s.file));
        }
    }
    assert!(bad.is_empty(), "{}:\n{}", bad.len(), bad.join("\n"));
}

/// Round I5 (the crate split): the core has no `web` feature, so no gate names it.
#[test]
#[ignore = "isolation: round I5"]
fn the_core_has_no_web_feature_gate() {
    let gate = Regex::new(r#"feature\s*=\s*"web""#).expect("regex");
    let bad: Vec<String> = sources()
        .iter()
        .filter(|s| s.module().is_some_and(|m| !is_interface(m)))
        .filter(|s| gate.is_match(&s.scan.full))
        .map(|s| format!("{}: {} gates", s.file, gate.find_iter(&s.scan.full).count()))
        .collect();
    assert!(bad.is_empty(), "{}:\n{}", bad.len(), bad.join("\n"));
}

/// The scanner is what every rule stands on, so it is tested while the rules are ignored.
#[test]
fn the_scanner_drops_comments_and_test_items_and_keeps_strings() {
    let scan = scan(
        "use crate::a; // crate::b\n/* crate::c /* nested */ */\n\
         #[cfg(test)]\nmod tests { fn f() { let _ = \"}\"; } use crate::d; }\n\
         fn g<'a>(x: &'a str) -> char { let _ = \"SELECT 1 FROM t\"; '}' }\n\
         #[cfg(test)]\nuse crate::e;\nuse crate::f;\n",
    );
    let uses: Vec<&str> = Regex::new(r"crate::(\w)")
        .expect("regex")
        .captures_iter(&scan.blank)
        .map(|m| m.get(1).expect("group").as_str())
        .collect();
    assert_eq!(uses, ["a", "f"], "{}", scan.blank);
    assert_eq!(scan.strings, ["SELECT 1 FROM t"]);
    assert!(Regex::new(SQL).expect("regex").is_match(&scan.strings[0]));
    assert!(scan.blank.contains("fn g<'a>") && !scan.blank.contains("SELECT"));
}

#[test]
fn the_rank_table_is_a_set_of_distinct_modules() {
    let names: BTreeSet<_> = RANKS.iter().map(|(m, _)| *m).collect();
    assert_eq!(names.len(), RANKS.len());
    assert!(RANKS.iter().all(|(m, _)| !is_interface(m)));
}
