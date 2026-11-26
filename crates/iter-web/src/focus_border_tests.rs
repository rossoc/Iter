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
    // the reset may be its own rule or share an `:is(.control,.path-pick)` list with R4's
    let inner: Vec<String> = rules(&source("ui/field.css"))
        .into_iter()
        .filter(|(list, _)| {
            selectors(list)
                .iter()
                .any(|s| is_bar_child_focus(s, ".control") && s.ends_with(":focus"))
        })
        .map(|(_, d)| d)
        .collect();
    assert!(
        !inner.is_empty(),
        "ui/field.css: no rule resets the input's own focus outline inside the bar"
    );
    for d in &inner {
        assert!(
            matches!(value(d, "outline"), Some("0" | "none")),
            "the input inside the bar draws its own line too: {d}"
        );
    }
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

/// The selectors of a selector list, split at its top-level commas (those inside `:is(...)`,
/// `:has(...)` and the like stay put).
fn selectors(list: &str) -> Vec<String> {
    let (mut out, mut depth, mut start) = (Vec::new(), 0usize, 0);
    for (i, c) in list.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                out.push(list[start..i].trim().to_string());
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(list[start..].trim().to_string());
    out
}

/// Whether `selector` picks the Pick button inside the folder-picker bar while it has focus:
/// `.field-row:has(.path-pick) <compound>` where the compound names `.path-pick` (on its own or
/// in an `:is(...)`/`:where(...)` list) and carries `:focus` or `:focus-visible`.
fn is_bar_button_focus(selector: &str) -> bool {
    is_bar_child_focus(selector, ".path-pick")
}

/// Whether `selector` picks a child of the folder-picker bar with class `class` while it has
/// focus (see [`is_bar_button_focus`]).
fn is_bar_child_focus(selector: &str, class: &str) -> bool {
    let Some(inner) = selector.strip_prefix(".field-row:has(.path-pick) ") else {
        return false;
    };
    let inner = inner.trim();
    !inner.contains(' ')
        && inner.contains(class)
        && (inner.ends_with(":focus") || inner.ends_with(":focus-visible"))
}

/// R4: inside the folder-picker bar the Pick button draws no line of its own when it has focus:
/// the global `:focus-visible` (base.css, `outline-offset:2px`) would put a ring outside the bar
/// next to the bar's own line. field.css resets it (`outline:0`/`none`) with a selector that out-
/// ranks base.css's bare `:focus-visible`. Fails today: only the bar's `.control:focus` is reset.
#[test]
fn the_folder_picker_button_draws_no_line_of_its_own() {
    // the ring it would draw otherwise: base.css's global focus cue sits outside the element
    let global = rule("base.css", ":focus-visible");
    assert!(
        value(&global, "outline-offset").is_some_and(|o| px(o) > 0.0),
        "base.css `:focus-visible` no longer draws outside the element: {global}"
    );

    let resets: Vec<(String, String)> = rules(&source("ui/field.css"))
        .into_iter()
        .filter(|(list, _)| selectors(list).iter().any(|s| is_bar_button_focus(s)))
        .collect();
    assert!(
        !resets.is_empty(),
        "ui/field.css: no rule resets the Pick button's focus outline inside the bar \
         (`.field-row:has(.path-pick) .path-pick:focus-visible` or the like)"
    );
    for (selector, declarations) in &resets {
        let outline = value(declarations, "outline");
        assert!(
            matches!(outline, Some("0" | "none")),
            "ui/field.css: `{selector}` leaves the Pick button its own line: {declarations}"
        );
    }
    // the bar keeps its own line, so the button still shows focus
    let bar = rule("ui/field.css", ".field-row:has(.path-pick):focus-within");
    assert_line_on_the_border(".field-row:has(.path-pick):focus-within", &bar);

    // the matcher finds the forms the plan allows and not the bar or the input
    assert!(is_bar_button_focus(
        ".field-row:has(.path-pick) .path-pick:focus-visible"
    ));
    assert!(is_bar_button_focus(
        ".field-row:has(.path-pick) :is(.control,.path-pick):focus"
    ));
    assert!(!is_bar_button_focus(
        ".field-row:has(.path-pick) .control:focus"
    ));
    assert!(!is_bar_button_focus(
        ".field-row:has(.path-pick):focus-within"
    ));
    assert!(!is_bar_button_focus(
        ".field-row:has(.path-pick) .path-pick:hover"
    ));
    assert_eq!(
        selectors(".a:is(.b,.c):focus,.d"),
        [".a:is(.b,.c):focus", ".d"]
    );
}
