//! The accessibility fixes of task 303 (R10, #169) and the run page that follows its log (R9,
//! #244), read from the sources the pages are built from: the components and their stylesheets.

use std::path::PathBuf;

fn source(path: &str) -> String {
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join(path);
    std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("{}: {e}", file.display()))
}

/// The `<summary>…</summary>` parts of a component's markup.
fn summaries(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("<summary") {
        let after = &rest[at..];
        let end = after
            .find("</summary>")
            .unwrap_or_else(|| panic!("an unclosed <summary>: {after}"));
        out.push(&after[..end]);
        rest = &after[end..];
    }
    out
}

/// R10: a `<summary>` is a button to a screen reader, so a heading inside it is lost (and the
/// button gets a heading's role mixed in). The folded and the unplaced lists keep their heading
/// out of it. Fails today: both put an `<h2>` in the summary.
#[test]
fn no_list_puts_a_heading_inside_its_summary() {
    for path in ["ui/folded_list.rs", "ui/unplaced_list.rs"] {
        let text = source(path);
        let found = summaries(&text);
        assert!(!found.is_empty(), "{path} still folds with a <summary>");
        for summary in found {
            for level in 1..=6 {
                assert!(
                    !summary.contains(&format!("<h{level}")),
                    "{path}: a heading inside <summary>: {summary}"
                );
            }
        }
    }
    // the reader of summaries finds what it should
    assert_eq!(
        summaries("<details><summary><h2>x</h2></summary></details>"),
        ["<summary><h2>x</h2>"]
    );
}

/// The `selector { declarations }` rules of a stylesheet, those inside `@media` included (with
/// their own selector); the `@` rules themselves are left out.
fn rules(css: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (open, _) in css.match_indices('{') {
        let before = &css[..open];
        let start = before.rfind(['{', '}']).map_or(0, |i| i + 1);
        let selector = before[start..].trim();
        // a comment before the selector
        let selector = selector
            .rsplit("*/")
            .next()
            .unwrap_or(selector)
            .trim()
            .to_string();
        if selector.starts_with('@') {
            continue;
        }
        let body = &css[open + 1..];
        let end = body.find('}').unwrap_or(body.len());
        out.push((selector, body[..end].to_string()));
    }
    out
}

/// The value of `property` in a rule's declarations, if it sets it.
fn value<'a>(declarations: &'a str, property: &str) -> Option<&'a str> {
    declarations.split(';').find_map(|d| {
        let (name, value) = d.split_once(':')?;
        (name.trim() == property).then(|| value.trim())
    })
}

/// R10: a card's action link (`.tcard-act`) stays faintly visible on a big screen instead of
/// `opacity:0`: a link no one can see until they hover is not found by sight. Fails today.
#[test]
fn a_cards_action_link_is_never_invisible() {
    let css = source("ui/task_card.css");
    let all = rules(&css);
    assert!(
        all.iter().any(|(s, _)| s.contains(".tcard-act")),
        "the card still has its action link: {all:?}"
    );
    for (selector, declarations) in &all {
        if !selector.contains(".tcard-act") {
            continue;
        }
        if let Some(opacity) = value(declarations, "opacity") {
            let n: f32 = opacity
                .trim_end_matches('%')
                .parse()
                .unwrap_or_else(|_| panic!("{selector}: opacity {opacity}"));
            assert!(n > 0.0, "{selector} hides the link: opacity {opacity}");
        }
    }
    // the reader of rules is right on a rule inside @media
    let sample = rules("@media (x){.a{opacity:0}\n.b:hover .a{opacity:1}}");
    assert_eq!(
        sample,
        [
            (".a".to_string(), "opacity:0".to_string()),
            (".b:hover .a".to_string(), "opacity:1".to_string())
        ]
    );
}

/// R10: the links of a card (its title) and of the breadcrumb get padding, so the pointer
/// target is 24px high, not just the line of text. Fails today: neither rule has padding.
#[test]
fn card_and_breadcrumb_links_are_padded_to_a_target() {
    for (path, selector) in [
        ("ui/breadcrumb.css", ".crumbs a"),
        ("ui/task_card.css", ".tcard-title"),
    ] {
        let css = source(path);
        let padded = rules(&css).into_iter().any(|(s, d)| {
            s.split(',').any(|one| one.trim() == selector)
                && ["padding", "padding-block", "min-height"]
                    .iter()
                    .any(|p| value(&d, p).is_some())
        });
        assert!(padded, "{path}: `{selector}` has no padding");
    }
}

/// R9: the page of a running agent follows its log by itself (the live version moves with the
/// log), so the Output tab has no Refresh link. Fails today.
#[test]
fn a_running_agents_output_has_no_refresh_link() {
    let text = source("pages/agent/agents.rs");
    assert!(
        text.contains("fn output_part"),
        "the Output tab is still built here"
    );
    assert!(
        !text.contains("\"Refresh\""),
        "pages/agent/agents.rs still renders a Refresh link"
    );
}
