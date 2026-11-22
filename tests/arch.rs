//! The architecture test (task 298, R5): the layering of sketch `architecture`, read from the
//! source text of `src/**/*.rs`. It runs no iter code and touches no tmux, config or database.
//!
//! Comments and `#[cfg(test)]` items are stripped before anything is counted, as `deps.py` does,
//! so a rule is about production code only. Each rule is its own test; while isolation is open
//! the ones not true yet carry `#[ignore = "isolation: round I<n>"]` (the round of sketch `plan`
//! that makes them true) and whoever makes a rule true removes its `ignore`; the others guard
//! what holds today. Run one with
//! `cargo test --test arch -- --ignored <name>`: it only reads files.

use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// The core modules and their rank: a module imports only modules of a lower rank (sketch
/// `architecture`, section 1). `agent` is the top of what the others may not reach.
const RANKS: [(&str, u8); 25] = [
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
#[ignore = "isolation: round I4"]
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
#[ignore = "isolation: round I4"]
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

// ---- Round I3 (isolation steps 2-4): the rules each step makes true -------------------------

/// The production source `file` (a path under `src/`, like `models/task_ref.rs`), if it exists.
fn file_of<'a>(sources: &'a [Source], file: &str) -> Option<&'a Source> {
    sources.iter().find(|s| s.file == file)
}

fn src_path(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(rel)
}

/// Every top-level module `module` names in production code.
fn imported_by(sources: &[Source], module: &str) -> BTreeSet<String> {
    imports(sources)
        .remove(module)
        .map(|to| to.into_keys().collect())
        .unwrap_or_default()
}

/// Names of the files (under `src/`) whose production text matches `pattern`.
fn files_matching(sources: &[Source], pattern: &str) -> Vec<String> {
    let re = Regex::new(pattern).expect("regex");
    sources
        .iter()
        .filter(|s| re.is_match(&s.scan.blank))
        .map(|s| s.file.clone())
        .collect()
}

fn none(what: &str, found: Vec<String>) {
    assert!(found.is_empty(), "{what}: {}", found.join(", "));
}

/// Step 2: `models` is a leaf over `error`, `text`, `files` and `process`: `check_url` is its own
/// (not `platform`'s) and the row types hold no dependency on the storage or the clients.
#[test]
fn models_imports_only_the_leaves() {
    let found = imported_by(&sources(), "models");
    let allowed: BTreeSet<String> = ["error", "text", "files", "process"]
        .map(String::from)
        .into();
    let extra: Vec<_> = found.difference(&allowed).cloned().collect();
    assert!(extra.is_empty(), "models imports {}", extra.join(", "));
}

/// Step 2: `#[derive(Table)]` emits paths the text scan above cannot see, so read the macro.
#[test]
fn the_table_derive_names_models_and_not_db() {
    let text = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("macros/src/lib.rs"))
        .expect("the macros crate");
    assert!(
        !text.contains("crate::db"),
        "the derive emits `crate::db::..`: an upward edge"
    );
    assert!(text.contains("crate::models::sqltypes"));
}

/// Step 2: the SQLite conversions live with the row types, and `md_edit` is split into `text` (the
/// pure half) and `editor` (the launch half).
#[test]
fn sqltypes_sit_in_models_and_md_edit_is_split() {
    assert!(
        src_path("models/sqltypes.rs").exists(),
        "models/sqltypes.rs"
    );
    assert!(!src_path("db/sqltypes.rs").exists(), "db/sqltypes.rs");
    assert!(src_path("models/traits.rs").exists(), "models/traits.rs");
    assert!(!src_path("md_edit.rs").exists(), "md_edit.rs");
}

/// Step 3: a library does not exit the process. `config::load` and `App::open` return errors and
/// only `main` (in the interface) turns one into an exit code.
#[test]
fn only_the_interface_exits_the_process() {
    let sources = sources();
    let found: Vec<String> = sources
        .iter()
        .filter(|s| s.module().is_none_or(|m| !is_interface(m)))
        .filter(|s| s.module().is_some())
        .filter(|s| s.scan.blank.contains("process::exit"))
        .map(|s| s.file.clone())
        .collect();
    none("exits the process", found);
}

/// Step 3: `open_db` and the exiting `App::new` are gone; `db` no longer reads the config.
#[test]
fn the_database_does_not_open_itself_from_the_config() {
    let sources = sources();
    none(
        "`open_db`/`App::new` remain",
        files_matching(&sources, r"\bfn open_db\b|\bApp::new\(|\bopen_db\("),
    );
    let found = imported_by(&sources, "db");
    for up in ["config", "app", "utils"] {
        assert!(!found.contains(up), "db imports {up}");
    }
}

/// Step 3: the secret store is told the old and the new name; it does not read a row.
#[test]
fn secret_does_not_read_the_database() {
    let found = imported_by(&sources(), "secret");
    assert!(!found.contains("db"), "secret imports db");
    assert!(!found.contains("models"), "secret imports models");
}

/// Step 3: what the interface asked `db` through a SQL filter, it asks through the typed
/// accessors of the design; the filter-taking one is not public.
#[test]
fn the_interface_asks_the_database_by_name_not_by_filter() {
    let sources = sources();
    let interface: Vec<_> = sources
        .iter()
        .filter(|s| s.module().is_some_and(is_interface))
        .collect();
    let found: Vec<_> = interface
        .iter()
        .filter(|s| s.scan.blank.contains("first_by_name"))
        .map(|s| s.file.clone())
        .collect();
    none("the interface passes a SQL filter", found);
    let db = sources
        .iter()
        .filter(|s| s.module() == Some("db"))
        .map(|s| s.scan.blank.as_str())
        .collect::<String>();
    for api in ["fn first<", "fn require<", "fn task_with_project"] {
        assert!(db.contains(api), "db lacks `{api}`");
    }
}

/// Step 4: `utils` is dissolved: `locate`, `workspace` and `reporting::gather` are out, and
/// `crud`/`output` wait for the `cli` module of step 6.
#[test]
fn utils_keeps_only_what_waits_for_the_cli() {
    let left: Vec<String> = fs::read_dir(src_path("utils"))
        .map(|dir| {
            dir.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|name| !["mod.rs", "crud.rs", "output.rs"].contains(&name.as_str()))
                .collect()
        })
        .unwrap_or_default();
    none("still in utils/", left);
    assert!(
        src_path("locate.rs").exists() || src_path("locate").is_dir(),
        "locate"
    );
    assert!(
        src_path("workspace.rs").exists() || src_path("workspace").is_dir(),
        "workspace"
    );
}

/// Step 4: `front` is what a front end shows, as data: four files out of `web`, with no markup,
/// no URL, no DOM id, and nothing above `models`.
#[test]
fn front_is_pure_data_moved_out_of_web() {
    let sources = sources();
    for (front, web) in [
        ("query", "task_query"),
        ("search", "search"),
        ("matrix", "quadrants"),
        ("day", "agenda_day"),
    ] {
        assert!(
            src_path(&format!("front/{front}.rs")).exists(),
            "front/{front}.rs"
        );
        assert!(!src_path(&format!("web/{web}.rs")).exists(), "web/{web}.rs");
    }
    assert!(
        src_path("web/folder_picker.rs").exists(),
        "the OS dialog stays in the web"
    );
    let markup = Regex::new(r"topcoat|view!|crate::web|\bhref\b").expect("regex");
    let mut bad = Vec::new();
    for s in sources.iter().filter(|s| s.module() == Some("front")) {
        if markup.is_match(&s.scan.blank) {
            bad.push(format!("{}: markup or a web import", s.file));
        }
        // a URL path or a DOM id written as a literal
        bad.extend(
            s.scan
                .strings
                .iter()
                .filter(|t| t.starts_with('/') || t.starts_with("q-") || t.starts_with("http"))
                .map(|t| format!("{}: {t:?}", s.file)),
        );
    }
    none("front is not pure data", bad);
    let found = imported_by(&sources, "front");
    let allowed: BTreeSet<String> = ["error", "models", "text"].map(String::from).into();
    let extra: Vec<_> = found.difference(&allowed).cloned().collect();
    assert!(extra.is_empty(), "front imports {}", extra.join(", "));
}

/// Task ids: the id is written and read in `models/task_ref.rs` only; its field is private, so no
/// caller builds one by hand.
#[test]
fn the_task_id_field_is_private_to_the_task_ref_module() {
    let sources = sources();
    let home = file_of(&sources, "models/task_ref.rs").expect("models/task_ref.rs");
    assert!(
        !Regex::new(r"struct TaskId\(\s*pub")
            .expect("regex")
            .is_match(&home.scan.blank),
        "`TaskId`'s field is public"
    );
    let elsewhere: Vec<String> = sources
        .iter()
        .filter(|s| s.file != home.file && s.scan.blank.contains("TaskId("))
        .map(|s| s.file.clone())
        .collect();
    none("builds or takes apart a `TaskId`", elsewhere);
}

/// Task ids: one grammar. No hand parser of `#<id>` beside `TaskRef`, and the name-keyed helpers
/// of sketch `architecture` section 5 are gone (`locate::task`/`task_in`, `Db::tasks_titled` and
/// `Db::task_labels` replace them; completion picks between the labels of one query).
#[test]
fn one_task_ref_grammar_and_no_name_keyed_helpers() {
    let sources = sources();
    let by_hand: Vec<String> = sources
        .iter()
        .filter(|s| s.file != "models/task_ref.rs")
        .filter(|s| s.scan.blank.contains("split_label"))
        .map(|s| s.file.clone())
        .collect();
    none("parses a label by hand", by_hand);
    none(
        "a name-keyed helper remains",
        files_matching(
            &sources,
            r"\bfn (task_ref|split_task_ref|parse_task_ref|task_display|find_task_or_err|find_tasks?|task_refs|task_names|resolve_task)\b",
        ),
    );
    let db: String = sources
        .iter()
        .filter(|s| s.module() == Some("db"))
        .map(|s| s.scan.blank.as_str())
        .collect();
    assert!(db.contains("fn tasks_titled"), "db lacks `tasks_titled`");
    assert!(db.contains("fn task_labels"), "db lacks `task_labels`");
}

/// The migration runner finds the rebuild step by what it is: no hand-kept index into the list.
#[test]
fn the_rebuild_step_has_no_hand_kept_index() {
    let sources = sources();
    let migrate = file_of(&sources, "db/migrate.rs").expect("db/migrate.rs");
    assert!(
        !Regex::new(r"const REBUILD\b")
            .expect("regex")
            .is_match(&migrate.scan.blank),
        "`REBUILD` is an index kept by hand"
    );
}

// ---- Round I4 (isolation steps 5-6): `agent::ingest_job` and the `cli` module -----------------

/// Every `.rs` file of `src/` as `(path under src, raw text)`, test files included: the rules below
/// are about where code that drives the command line may sit, and its tests count.
fn raw_files() -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut all = Vec::new();
    fn go(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in fs::read_dir(dir).expect("a readable folder").flatten() {
            let path = e.path();
            if path.is_dir() {
                if e.file_name() != "graphify-out" {
                    go(&path, out);
                }
            } else if path.extension().is_some_and(|x| x == "rs") {
                out.push(path);
            }
        }
    }
    go(&root, &mut all);
    all.sort();
    all.into_iter()
        .map(|p| {
            let rel = p
                .strip_prefix(&root)
                .expect("under src")
                .display()
                .to_string();
            (rel, fs::read_to_string(&p).expect("a readable file"))
        })
        .collect()
}

/// Step 5: the ingest job runs an agent, so it sits in `agent`; `wiki` reads markdown and knows no
/// agent.
#[test]
fn the_ingest_job_is_in_agent_and_wiki_knows_no_agent() {
    assert!(src_path("agent/ingest_job").is_dir(), "agent/ingest_job/");
    assert!(!src_path("wiki/ingest_job").exists(), "wiki/ingest_job");
    let found = imported_by(&sources(), "wiki");
    assert!(!found.contains("agent"), "wiki imports agent");
}

/// Step 6: `args`, `commands`, `completion`, `crud` and `output` are one module `cli`, with the
/// editor (only the command line launches one) and the tests that drive `Args` inside it.
#[test]
fn the_command_line_is_one_cli_module() {
    for old in [
        "args",
        "commands",
        "completion.rs",
        "utils",
        "editor.rs",
        "agent/config_instructions_tests.rs",
    ] {
        assert!(!src_path(old).exists(), "{old} is still there");
    }
    for new in [
        "cli/args/mod.rs",
        "cli/run.rs",
        "cli/complete.rs",
        "cli/entity.rs",
        "cli/output.rs",
        "cli/editor.rs",
        "cli/config_instructions_tests.rs",
    ] {
        assert!(src_path(new).exists(), "{new} is missing");
    }
}

/// Step 6: the core cannot name the command line, not even in a test: no `Args::try_parse_from`,
/// no `crate::args`/`crate::cli`/`crate::commands` outside the interface. The argv a core module
/// builds for the CLI (`worker_args`, `ingest_args`, `hook_args`) is parsed by a test in `cli`.
#[test]
fn nothing_but_the_interface_names_the_command_line() {
    let named =
        Regex::new(r"try_parse_from|crate::(args|cli|commands|completion)\b").expect("regex");
    let found: Vec<String> = raw_files()
        .into_iter()
        .filter(|(file, _)| {
            !(file.starts_with("cli/") || file.starts_with("web/") || file == "main.rs")
        })
        .filter(|(_, text)| named.is_match(text))
        .map(|(file, _)| file)
        .collect();
    none("the core names the command line", found);
    let cli: String = raw_files()
        .into_iter()
        .filter(|(file, _)| file.starts_with("cli/"))
        .map(|(_, text)| text)
        .collect();
    for argv in ["worker_args", "ingest_args", "hook_args"] {
        assert!(
            cli.contains(argv) && cli.contains("try_parse_from"),
            "no test in cli/ parses `{argv}`"
        );
    }
}

/// Step 6: the slug is the task's (`Task::slug`), not a `git` function of a title and an id.
#[test]
fn the_task_slug_is_a_task_method() {
    assert!(
        none_of(&sources(), r"\bfn task_slug\b|\btask_slug\("),
        "`git::task_slug` remains"
    );
}

fn none_of(sources: &[Source], pattern: &str) -> bool {
    files_matching(sources, pattern).is_empty()
}

/// Step 6: migration steps say `copy_first` themselves; the runner does not compare function
/// addresses.
#[test]
fn migration_steps_carry_copy_first_and_nothing_compares_function_addresses() {
    let sources = sources();
    let migrate = file_of(&sources, "db/migrate.rs").expect("db/migrate.rs");
    assert!(migrate.scan.blank.contains("copy_first"), "no `copy_first`");
    none(
        "compares function addresses",
        files_matching(&sources, r"fn_addr_eq|ptr::eq\(.*step"),
    );
}

/// Step 6: the web's DOM id of a quadrant is a function of the quadrant, not an array kept in step
/// with `QUADRANTS` by hand.
#[test]
fn a_quadrants_dom_id_comes_from_a_function_not_a_parallel_array() {
    let sources = sources();
    none(
        "a parallel array of DOM ids",
        files_matching(&sources, r"\bDOM_IDS\b"),
    );
    let web: String = sources
        .iter()
        .filter(|s| s.file.starts_with("web/"))
        .map(|s| s.scan.full.as_str())
        .collect();
    assert!(
        Regex::new(r"fn \w*dom_id\w*\(")
            .expect("regex")
            .is_match(&web),
        "no `fn ..dom_id..` in web"
    );
}

/// Step 6: `locate::task_in` reads an id through the `TaskRef`/`TaskId` grammar only: no `#` test
/// written by hand in `locate.rs` (other modules strip `#` for tags and notes, which is not an id).
#[test]
fn task_in_has_no_hash_check_of_its_own() {
    // char literals are blank in the scan, so read the text; a comment line is no code
    let hash = Regex::new(r"(starts_with|trim_start_matches|strip_prefix)\('#'\)").expect("regex");
    let found: Vec<String> = raw_files()
        .into_iter()
        .filter(|(file, _)| file == "locate.rs")
        .filter(|(_, text)| {
            text.lines()
                .filter(|l| !l.trim_start().starts_with("//"))
                .any(|l| hash.is_match(l))
        })
        .map(|(file, _)| file)
        .collect();
    none("reads a `#<id>` by hand", found);
}

/// Step 6: `session new` and the adoption of an agent's workspace are one path: the second
/// function is gone and the "could not attach" advice is written once.
#[test]
fn session_new_and_an_adopted_workspace_share_one_path() {
    let sources = sources();
    none(
        "`adopt_workspace` remains",
        files_matching(&sources, r"\bfn adopt_workspace\b"),
    );
    let twice: Vec<String> = raw_files()
        .into_iter()
        .filter(|(file, _)| file.starts_with("cli/") || file.starts_with("commands/"))
        .filter(|(_, text)| text.matches("the session is set up -- attach with").count() > 1)
        .map(|(file, _)| file)
        .collect();
    none("the attach advice is written twice", twice);
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
