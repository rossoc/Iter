//! How `ui/live.js` follows the server (R2e-R2g of task 302), run under node against the small
//! hand-made DOM, fake stream, fetch and clock in `tests/live_js/harness.js`. Each test is one
//! scenario of the harness; it is skipped (and says so) when `node` is not installed.

use std::{io::ErrorKind, path::PathBuf, process::Command};

/// Runs one scenario of the harness against `src/ui/live.js`; passes when it prints `ok`.
fn scenario(name: &str) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let harness = dir.join("tests/live_js/harness.js");
    let script = dir.join("src/ui/live.js");
    let out = match Command::new("node")
        .arg(&harness)
        .arg(&script)
        .arg(name)
        .output()
    {
        Ok(out) => out,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            eprintln!("node is not installed: the live.js scenario {name} is skipped");
            return;
        }
        Err(e) => panic!("node runs: {e}"),
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success() && stdout.trim_end().ends_with("ok"),
        "live.js, {name}:\n{stdout}{stderr}"
    );
}

/// The harness has exactly the scenarios below, so none is left without a test.
#[test]
fn every_scenario_of_the_harness_has_a_test() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let Ok(out) = Command::new("node")
        .arg(dir.join("tests/live_js/harness.js"))
        .arg(dir.join("src/ui/live.js"))
        .arg("--list")
        .output()
    else {
        eprintln!("node is not installed: skipped");
        return;
    };
    let listed: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect();
    let mut tested = SCENARIOS.map(str::to_string).to_vec();
    tested.sort();
    let mut listed_sorted = listed.clone();
    listed_sorted.sort();
    assert_eq!(listed_sorted, tested);
}

const SCENARIOS: [&str; 13] = [
    "the_page_follows_a_new_version",
    "held_by_a_field_applies_when_focus_leaves",
    "held_by_an_edited_form_applies_on_reset",
    "held_by_an_edited_form_applies_on_submit",
    "held_by_a_pop_up_applies_when_it_closes",
    "the_toast_does_not_ask_for_a_reload",
    "focus_on_a_link_does_not_hold_and_comes_back_by_id",
    "focus_comes_back_by_href",
    "focus_comes_back_by_name_and_value",
    "focus_falls_back_to_the_region",
    "a_dropped_stream_reconnects_and_catches_up",
    "a_reconnecting_stream_catches_up",
    "a_hidden_tab_closes_its_stream_and_catches_up_when_shown",
];

/// Guard: with nothing held, a new version is swapped in and no toast is raised.
#[test]
fn the_page_follows_a_new_version() {
    scenario("the_page_follows_a_new_version");
}

/// R2e: a swap held while a field had focus is applied once focus leaves the field, with no
/// further change on the server. Fails today: the held swap waits for the next change.
#[test]
fn held_by_a_field_applies_when_focus_leaves() {
    scenario("held_by_a_field_applies_when_focus_leaves");
}

/// R2e: a swap held for an edited form is applied once the form is reset. Fails today.
#[test]
fn held_by_an_edited_form_applies_on_reset() {
    scenario("held_by_an_edited_form_applies_on_reset");
}

/// R2e: a swap held for an edited form is applied once the form is submitted and the page stays
/// (a script handled the submit). Fails today.
#[test]
fn held_by_an_edited_form_applies_on_submit() {
    scenario("held_by_an_edited_form_applies_on_submit");
}

/// R2e: a swap held while a pop-up was open is applied once it closes. Fails today.
#[test]
fn held_by_a_pop_up_applies_when_it_closes() {
    scenario("held_by_a_pop_up_applies_when_it_closes");
}

/// R2e: the toast for a held swap no longer asks for a reload. Fails today: "Reload to see the
/// latest."
#[test]
fn the_toast_does_not_ask_for_a_reload() {
    scenario("the_toast_does_not_ask_for_a_reload");
}

/// R2f: focus on a link does not hold the swap, and comes back to the fresh link with the same
/// id. Fails today: any focus in the region holds the swap.
#[test]
fn focus_on_a_link_does_not_hold_and_comes_back_by_id() {
    scenario("focus_on_a_link_does_not_hold_and_comes_back_by_id");
}

/// R2f: a link without an id gets focus back by its href. Fails today (held).
#[test]
fn focus_comes_back_by_href() {
    scenario("focus_comes_back_by_href");
}

/// R2f: a button without an id gets focus back by its name and value. Fails today (held).
#[test]
fn focus_comes_back_by_name_and_value() {
    scenario("focus_comes_back_by_name_and_value");
}

/// R2f: when the focused element is not in the fresh page, focus goes to the region. Fails
/// today (held).
#[test]
fn focus_falls_back_to_the_region() {
    scenario("focus_falls_back_to_the_region");
}

/// R2g: a stream that closed is opened again after a backoff; the same version leaves the page,
/// a different one refreshes it. Fails today: a closed stream stays closed.
#[test]
fn a_dropped_stream_reconnects_and_catches_up() {
    scenario("a_dropped_stream_reconnects_and_catches_up");
}

/// R2g: a stream the browser reconnects itself still catches up. Passes today (a guard).
#[test]
fn a_reconnecting_stream_catches_up() {
    scenario("a_reconnecting_stream_catches_up");
}

/// R2g: a hidden tab closes its stream and catches up when shown. Passes today (a guard).
#[test]
fn a_hidden_tab_closes_its_stream_and_catches_up_when_shown() {
    scenario("a_hidden_tab_closes_its_stream_and_catches_up_when_shown");
}
