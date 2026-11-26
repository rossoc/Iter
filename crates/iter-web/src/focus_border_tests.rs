//! The active form's border (task 320): a focused control shows one thick (2px) accent line on
//! its own border and no ring outside it. Read from the stylesheets the controls are drawn with.

use std::path::PathBuf;

fn source(path: &str) -> String {
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join(path);
    std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("{}: {e}", file.display()))
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

/// The declarations of the rule whose selector is exactly `selector`.
fn rule(path: &str, selector: &str) -> String {
    rules(&source(path))
        .into_iter()
        .find(|(s, _)| s == selector)
        .map(|(_, d)| d)
        .unwrap_or_else(|| panic!("{path}: no rule `{selector}`"))
}

/// An `outline-offset` in px as a number (`0` and `-2px` included).
fn px(offset: &str) -> f64 {
    offset
        .trim()
        .trim_end_matches("px")
        .parse()
        .unwrap_or_else(|_| panic!("an outline-offset not in px: {offset}"))
}

/// A focus line that is one 2px accent outline drawn over the border: a real outline (forced
/// colors keeps it), 2px, the accent, and pulled in by its own width so nothing lies outside.
fn assert_line_on_the_border(what: &str, declarations: &str) {
    let outline = value(declarations, "outline")
        .unwrap_or_else(|| panic!("{what}: no outline ({declarations})"));
    assert!(
        outline.contains("2px") && outline.contains("solid") && outline.contains("var(--accent)"),
        "{what}: the focus line is not a 2px solid accent outline: {outline}"
    );
    assert_eq!(
        value(declarations, "outline-offset"),
        Some("-2px"),
        "{what}: the focus line does not lie over the border (an outer ring): {declarations}"
    );
}

/// R1: a focused `.control` (input, textarea, select, combobox button, filter search) draws its
/// 2px accent line over its own border and turns the border accent. Fails today: the outline
/// sits 1px outside the border (`outline-offset:1px`), a second ring round the control.
#[test]
fn a_focused_control_draws_its_line_on_its_border() {
    let focus = rule("ui/control.css", ".control:focus");
    assert_line_on_the_border(".control:focus", &focus);
    assert_eq!(
        value(&focus, "border-color"),
        Some("var(--accent)"),
        ".control:focus: the border does not turn accent: {focus}"
    );
    assert_eq!(
        value(&focus, "box-shadow"),
        None,
        ".control:focus: a shadow ring besides the outline: {focus}"
    );
}

/// R2: the folder-picker bar (input and folder button as one bar) draws the same line over the
/// bar's border while either has focus, and the input inside draws none of its own. Fails
/// today: the bar's outline sits 1px outside it (`outline-offset:1px`).
#[test]
fn the_folder_picker_bar_draws_its_line_on_its_border() {
    let bar = rule("ui/field.css", ".field-row:has(.path-pick):focus-within");
    assert_line_on_the_border(".field-row:has(.path-pick):focus-within", &bar);
    let inner = rule("ui/field.css", ".field-row:has(.path-pick) .control:focus");
    assert_eq!(
        value(&inner, "outline"),
        Some("0"),
        "the input inside the bar draws its own line too: {inner}"
    );
}

/// R3: no focus rule of the form controls puts a line outside the control (a positive
/// `outline-offset` or a shadow), and the invalid control keeps its 2px danger border. Fails
/// today: both focus rules set `outline-offset:1px`.
#[test]
fn no_focus_rule_draws_a_ring_outside_a_control() {
    let mut focus_rules = 0;
    for path in ["ui/control.css", "ui/field.css"] {
        for (selector, declarations) in rules(&source(path)) {
            if !selector.contains(":focus") {
                continue;
            }
            focus_rules += 1;
            if let Some(offset) = value(&declarations, "outline-offset") {
                assert!(
                    px(offset) <= 0.0,
                    "{path}: `{selector}` draws its focus line {offset} outside the control"
                );
            }
            assert_eq!(
                value(&declarations, "box-shadow"),
                None,
                "{path}: `{selector}` draws a shadow ring: {declarations}"
            );
        }
    }
    // .control:focus, the bar's inner .control:focus and the bar's :focus-within
    assert!(focus_rules >= 3, "only {focus_rules} focus rules found");

    let invalid = rule("ui/control.css", ".control[aria-invalid=true]");
    assert_eq!(value(&invalid, "border-width"), Some("2px"), "{invalid}");
    assert_eq!(
        value(&invalid, "border-color"),
        Some("var(--danger)"),
        "{invalid}"
    );

    // the reader of offsets finds what it should
    assert!(px("1px") > 0.0);
    assert!(px("-2px") < 0.0);
    assert_eq!(px("0"), 0.0);
}
