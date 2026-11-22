//! The architecture of the crates: the manifests, the names each crate may use, and, in
//! [`layers`], the ranks of the core modules and the placement
//! rules of the isolation steps. It reads the three manifests and `crates/*/src/**/*.rs` as text
//! and runs no iter code, so it touches no tmux, config or database.

#[path = "arch/layers.rs"]
mod layers;

use std::fs;
use std::path::{Path, PathBuf};

/// The workspace root, two levels above `iter-cli`.
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn crate_dir(name: &str) -> PathBuf {
    root().join("crates").join(name)
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("{} cannot be read: {e}", path.display()))
}

fn manifest(name: &str) -> String {
    read(&crate_dir(name).join("Cargo.toml"))
}

/// The lines of `[header]` (the table named exactly), without comments and blanks.
fn table(toml: &str, header: &str) -> Vec<String> {
    let mut inside = false;
    let mut lines = Vec::new();
    for line in toml.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            inside = line == format!("[{header}]");
            continue;
        }
        let line = line.split('#').next().unwrap_or("").trim();
        if inside && !line.is_empty() {
            lines.push(line.to_string());
        }
    }
    lines
}

/// The key of a `key = value` or `key.workspace = true` line.
fn key(line: &str) -> &str {
    line.split(['=', '.']).next().unwrap_or("").trim()
}

fn dependency<'a>(deps: &'a [String], name: &str) -> Option<&'a String> {
    deps.iter().find(|l| key(l) == name)
}

fn sources(crate_name: &str) -> Vec<(PathBuf, String)> {
    fn walk(dir: &Path, out: &mut Vec<(PathBuf, String)>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        let mut entries: Vec<_> = entries.flatten().map(|e| e.path()).collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push((path.clone(), read(&path)));
            }
        }
    }
    let mut out = Vec::new();
    walk(&crate_dir(crate_name).join("src"), &mut out);
    assert!(
        !out.is_empty(),
        "crates/{crate_name}/src holds no Rust file: the crate is not split out yet"
    );
    out
}

/// Production code and tests alike: a name that must not appear anywhere in the crate. A crate
/// is named by a path (`iter_web::serve`): the bare word also starts `@iter_client_session_`.
fn files_naming<'a>(files: &'a [(PathBuf, String)], word: &str) -> Vec<&'a Path> {
    let path = format!("{word}::");
    files
        .iter()
        .filter(|(_, text)| text.contains(&path))
        .map(|(p, _)| p.as_path())
        .collect()
}

#[test]
fn the_workspace_is_the_three_crates_and_the_macros() {
    for name in ["iter-macros", "iter-core", "iter-cli", "iter-web"] {
        assert!(
            crate_dir(name).join("Cargo.toml").is_file(),
            "crates/{name}/Cargo.toml is missing"
        );
    }
    let root = read(&root().join("Cargo.toml"));
    let workspace = table(&root, "workspace");
    assert!(
        workspace.iter().any(|l| l == r#"members = ["crates/*"]"#),
        "members: {workspace:?}"
    );
    assert!(
        workspace
            .iter()
            .any(|l| l == r#"default-members = ["crates/iter-cli"]"#),
        "default-members: {workspace:?}"
    );
    assert!(
        workspace
            .iter()
            .any(|l| l.starts_with("resolver") && l.contains('3')),
        "resolver 3: {workspace:?}"
    );
    assert!(
        table(&root, "profile.release")
            .iter()
            .any(|l| l.starts_with("incremental")),
        "profiles live at the root"
    );
    assert!(
        !root.lines().any(|l| l.trim() == "[package]"),
        "the root is a virtual workspace"
    );
}

#[test]
fn the_core_depends_on_neither_interface_and_has_no_web_feature() {
    let toml = manifest("iter-core");
    let deps = table(&toml, "dependencies");
    for banned in [
        "iter-web",
        "iter-cli",
        "topcoat",
        "tokio",
        "pulldown-cmark",
        "clap_complete",
    ] {
        assert!(
            dependency(&deps, banned).is_none(),
            "iter-core depends on {banned}"
        );
    }
    let clap = dependency(&deps, "clap")
        .expect("iter-core keeps clap, optional, for the ValueEnum derives");
    assert!(
        clap.contains("optional") || clap.contains("workspace"),
        "{clap}"
    );
    let features = table(&toml, "features");
    assert!(
        features
            .iter()
            .any(|l| l.starts_with("clap") && l.contains("dep:clap")),
        "feature clap: {features:?}"
    );
    assert!(
        features.iter().any(|l| key(l) == "test-support"),
        "feature test-support: {features:?}"
    );
    assert!(
        !features.iter().any(|l| key(l) == "web"),
        "iter-core has no web feature: {features:?}"
    );
    for owned in ["rusqlite", "oxigraph", "ureq"] {
        assert!(dependency(&deps, owned).is_some(), "iter-core owns {owned}");
    }
}

#[test]
fn the_web_crate_does_not_know_the_cli_or_the_storage_crates() {
    let toml = manifest("iter-web");
    let deps = table(&toml, "dependencies");
    for banned in [
        "iter-cli",
        "rusqlite",
        "oxigraph",
        "ureq",
        "clap",
        "clap_complete",
    ] {
        assert!(
            dependency(&deps, banned).is_none(),
            "iter-web depends on {banned}"
        );
    }
    for needed in ["iter-core", "topcoat", "tokio", "pulldown-cmark"] {
        assert!(
            dependency(&deps, needed).is_some(),
            "iter-web needs {needed}"
        );
    }
}

#[test]
fn the_cli_is_the_iter_binary_and_reaches_the_server_only_through_its_feature() {
    let toml = manifest("iter-cli");
    let deps = table(&toml, "dependencies");
    for banned in [
        "rusqlite",
        "oxigraph",
        "ureq",
        "topcoat",
        "tokio",
        "pulldown-cmark",
    ] {
        assert!(
            dependency(&deps, banned).is_none(),
            "iter-cli depends on {banned}"
        );
    }
    let web = dependency(&deps, "iter-web").expect("iter-cli names iter-web");
    assert!(
        web.contains("optional = true"),
        "iter-web is optional: {web}"
    );
    assert!(
        dependency(&deps, "iter-core").is_some_and(|l| l.contains("clap")),
        "iter-core with feature clap"
    );
    let features = table(&toml, "features");
    assert!(
        features
            .iter()
            .any(|l| l.starts_with("default") && l.contains("web")),
        "default has web: {features:?}"
    );
    assert!(
        features
            .iter()
            .any(|l| l.starts_with("web") && l.contains("dep:iter-web")),
        "{features:?}"
    );
    let bin = table_array(&toml, "bin");
    assert!(
        bin.iter().any(|l| l.replace(' ', "") == r#"name="iter""#),
        "the binary is `iter`: {bin:?}"
    );
}

/// The lines of the first `[[header]]` array table.
fn table_array(toml: &str, header: &str) -> Vec<String> {
    let mut inside = false;
    let mut lines = Vec::new();
    for line in toml.lines().map(str::trim) {
        if line.starts_with('[') {
            inside = line == format!("[[{header}]]");
        } else if inside && !line.is_empty() {
            lines.push(line.to_string());
        }
    }
    lines
}

#[test]
fn the_core_source_has_no_web_gate_and_clap_only_in_its_derives() {
    let files = sources("iter-core");
    let gated: Vec<_> = files_naming(&files, r#"feature = "web""#);
    assert!(
        gated.is_empty(),
        r#"`feature = "web"` in the core: {gated:?}"#
    );
    let mut clap_lines = Vec::new();
    for (path, text) in &files {
        for line in text.lines().filter(|l| l.contains("clap")) {
            clap_lines.push((path.display().to_string(), line.trim().to_string()));
        }
    }
    let stray: Vec<_> = clap_lines
        .iter()
        .filter(|(_, l)| !l.contains(r#"cfg_attr(feature = "clap""#))
        .collect();
    assert!(stray.is_empty(), "clap outside a cfg_attr line: {stray:?}");
    assert_eq!(
        clap_lines.len(),
        4,
        "ValueEnum on Format, Depth, Harness and State: {clap_lines:?}"
    );
}

#[test]
fn only_main_names_the_server_crate_and_nothing_names_a_sibling_interface() {
    let cli = sources("iter-cli");
    let named: Vec<_> = files_naming(&cli, "iter_web");
    assert_eq!(
        named
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect::<Vec<_>>(),
        ["main.rs"],
        "iter_web is named only in cli/main.rs: {named:?}"
    );
    let core = sources("iter-core");
    for (word, files) in [
        ("iter_web", &core),
        ("iter_cli", &core),
        ("iter_cli", &sources("iter-web")),
    ] {
        let found = files_naming(files, word);
        assert!(found.is_empty(), "{word} named in {found:?}");
    }
}

#[test]
fn the_views_carry_the_recursion_limit_and_the_binary_does_not() {
    let web = read(&crate_dir("iter-web").join("src/lib.rs"));
    assert!(
        web.contains("recursion_limit"),
        "the topcoat views need the limit in iter-web"
    );
    let main = read(&crate_dir("iter-cli").join("src/main.rs"));
    assert!(
        !main.contains("recursion_limit"),
        "main.rs no longer holds the views"
    );
}

#[test]
fn the_web_crate_exports_serve_and_nothing_else() {
    let lib = read(&crate_dir("iter-web").join("src/lib.rs"));
    let public: Vec<_> = lib
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("pub "))
        .collect();
    assert_eq!(public.len(), 1, "one public item: {public:?}");
    assert!(public[0].starts_with("pub fn serve("), "{public:?}");
}
