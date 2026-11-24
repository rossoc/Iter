//! Page scripts keep working after a live swap (R2d of task 302). `live.js` replaces the children
//! of every `[data-live]` region (`<main id="main">` is one) with fresh ones, so a listener a
//! script added at load to an element in the page is gone with that element. A script that binds
//! listeners to elements it looked up must arm the fresh ones again on `iter:live` (the event
//! `live.js` sends after a swap), and mark what it armed so nothing is armed twice. A script that
//! listens only on `document` or `window` (delegation) keeps working and is not asked to.
//!
//! The scripts are read from the source folder, so a new one is checked without being listed.
//! The few that bind only outside the live regions are named below, and the reason each is left
//! out is checked against the rendered page and `live.js`.

use super::toast_tests::{get, isolated_in};
use std::path::{Path, PathBuf};
use topcoat::router::StatusCode;

/// Every `.js` file under `src`, by its path from `src`.
fn scripts() -> Vec<(String, String)> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, String)>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .expect("the source folder reads")
            .map(|e| e.expect("an entry").path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                walk(&path, root, out);
            } else if path.extension().is_some_and(|e| e == "js") {
                let name = path
                    .strip_prefix(root)
                    .expect("under src")
                    .to_string_lossy()
                    .replace('\\', "/");
                let text = std::fs::read_to_string(&path).expect("the script reads");
                out.push((name, text));
            }
        }
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut out = Vec::new();
    walk(&root, &root, &mut out);
    out
}

/// What each `.addEventListener(` is called on, when it is not `document` or `window`: the
/// expression just before it (`form`, `this.el`, `root.querySelector(...)` as `)`).
fn element_bindings(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (at, _) in text.match_indices(".addEventListener(") {
        let before = &text[..at];
        let start = before
            .char_indices()
            .rev()
            .take_while(|(_, c)| c.is_alphanumeric() || matches!(c, '_' | '$' | '.'))
            .last()
            .map_or(at, |(i, _)| i);
        let receiver = &before[start..];
        let last = receiver.rsplit('.').next().unwrap_or_default();
        if receiver.is_empty() {
            // a call or an index just before: `root.querySelector("x").addEventListener(`
            found.push(before.chars().last().map(String::from).unwrap_or_default());
        } else if last != "document" && last != "window" {
            found.push(receiver.to_string());
        }
    }
    found
}

/// Whether the script handles `iter:live` with a listener.
fn rearms_on_live(text: &str) -> bool {
    text.contains(r#"addEventListener("iter:live""#) || text.contains("addEventListener('iter:live'")
}

/// Whether the script marks what it armed, so arming again on `iter:live` skips it: a
/// `dataset.<flag> = ` it also reads, or a `WeakSet`/`WeakMap` of armed elements.
fn marks_what_it_armed(text: &str) -> bool {
    if text.contains("WeakSet") || text.contains("WeakMap") {
        return true;
    }
    text.match_indices("dataset.").any(|(at, _)| {
        let rest = &text[at + "dataset.".len()..];
        let flag: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        let after = rest[flag.len()..].trim_start();
        !flag.is_empty()
            && after.starts_with('=')
            && !after.starts_with("==")
            && text.matches(&format!("dataset.{flag}")).count() >= 2
    })
}

/// The scripts that bind elements outside every live region, and why that holds; each reason is
/// checked in `the_scripts_left_out_bind_only_outside_the_live_regions`.
const OUTSIDE: [(&str, &str); 4] = [
    (
        "ui/live.js",
        "it is the swap itself; its listeners are on its own event stream",
    ),
    (
        "ui/modal.js",
        "live.js does not swap while a pop-up (.modal-box) is open",
    ),
    (
        "ui/search.js",
        "the palette and its open button are outside <main>",
    ),
    ("ui/toast.js", "the toast region is outside <main>"),
];

/// R2d: every script that adds listeners to elements it looked up arms them again on `iter:live`,
/// and marks what it armed. Fails today on `pages/org/org.js` (the Report range form, the Share
/// form) and `pages/org/platform.js` (the activity list), which arm once at load.
#[test]
fn every_page_script_that_binds_elements_rearms_on_live() {
    let all = scripts();
    for wanted in ["pages/org/org.js", "pages/org/platform.js", "ui/combobox.js"] {
        assert!(
            all.iter().any(|(name, _)| name == wanted),
            "{wanted} is found among {:?}",
            all.iter().map(|(n, _)| n).collect::<Vec<_>>()
        );
    }
    // the reader of bindings is right on the shapes the scripts use
    assert_eq!(
        element_bindings(
            r#"document.addEventListener("x", f); window.addEventListener("y", f);
               form.addEventListener("z", f); this.el.addEventListener("z", f);
               root.querySelector("a").addEventListener("z", f);"#
        ),
        vec!["form", "this.el", ")"]
    );

    let mut stale = Vec::new();
    for (name, text) in &all {
        if OUTSIDE.iter().any(|(out, _)| out == name) {
            continue;
        }
        let bound = element_bindings(text);
        if bound.is_empty() {
            continue;
        }
        if !rearms_on_live(text) {
            stale.push(format!(
                "{name} binds {bound:?} once at load and does not arm again on iter:live"
            ));
        } else if !marks_what_it_armed(text) {
            stale.push(format!(
                "{name} arms again on iter:live but does not mark what it armed (data-armed)"
            ));
        }
    }
    assert!(stale.is_empty(), "{}", stale.join("\n"));
}

/// R2d: the scripts left out of the rule above bind only to elements a swap never replaces: the
/// palette, its button and the toast region are outside `<main>` on every page, and `live.js`
/// holds the swap while a pop-up is open. Passes today; it fails when one of them moves into a
/// live region, or a new script is left out without a reason.
#[test]
fn the_scripts_left_out_bind_only_outside_the_live_regions() {
    if isolated_in(
        module_path!(),
        "the_scripts_left_out_bind_only_outside_the_live_regions",
        None,
    ) {
        return;
    }
    let all = scripts();
    for (name, why) in OUTSIDE {
        assert!(
            all.iter().any(|(n, _)| n == name),
            "{name} ({why}) still exists"
        );
    }
    let live = &all.iter().find(|(n, _)| n == "ui/live.js").expect("live.js").1;
    assert!(
        live.contains("modal-box"),
        "live.js holds the swap while a pop-up is open"
    );

    for path in ["/", "/settings", "/agents"] {
        let page = get(path);
        assert_eq!(page.status, StatusCode::OK, "{path}: {}", page.body);
        let open = page.body.find("<main").expect("a <main>");
        let close = page.body.find("</main>").expect("a </main>");
        for marker in [r#"id="palette""#, "data-palette-open", r#"id="toasts""#] {
            let at = page
                .body
                .find(marker)
                .unwrap_or_else(|| panic!("{path} has {marker}"));
            assert!(
                at < open || at > close,
                "{path}: {marker} is outside <main>, so a swap never replaces it"
            );
        }
    }
}
