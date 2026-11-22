//! What the two task pop-ups (`task_notes.rs`, `task_sessions.rs`) put on the page, pinned before
//! their shared shape is merged (task 298, R7: "task modals"). The markup they must keep: the
//! modal's id and title, the form's action, its fields, the submit label and the way back.
//!
//! Like `toast_tests`, each test runs again in a child process whose config folder and database
//! are a scratch folder, so the user's own are never read or written.

use super::toast_tests::{get, isolated_in};
use iter_core::{config::config, db::Db, fixtures};

/// The page of task 1 with `tab` and `new` in its query, in a scratch database holding it.
fn modal_page(tab: &str, new: &str) -> String {
    let db = Db::open(config().db_path().expect("a scratch database path")).expect("opens");
    let project = fixtures::add_project(&db, "app");
    assert_eq!(fixtures::add_task(&db, project, "Fix it"), 1);
    drop(db);
    let page = get(&format!("/task/1?tab={tab}&new={new}"));
    assert_eq!(page.status.as_u16(), 200, "{}", page.body);
    page.body
}

#[test]
fn the_note_pop_up_asks_for_a_name_and_a_text() {
    if isolated_in(
        module_path!(),
        "the_note_pop_up_asks_for_a_name_and_a_text",
        None,
    ) {
        return;
    }
    let page = modal_page("notes", "note");
    for wanted in [
        r#"id="note-title""#,
        "New note",
        r#"action="/task/1/notes""#,
        r#"name="name""#,
        r#"name="text""#,
        "Add note",
        "/task/1?tab=notes",
    ] {
        assert!(page.contains(wanted), "{wanted} in the page:\n{page}");
    }
    assert!(!page.contains("New session"), "only the one pop-up");
}

#[test]
fn the_session_pop_up_asks_for_two_moments_and_a_note() {
    if isolated_in(
        module_path!(),
        "the_session_pop_up_asks_for_two_moments_and_a_note",
        None,
    ) {
        return;
    }
    let page = modal_page("sessions", "session");
    for wanted in [
        r#"id="session-title""#,
        "New session",
        r#"action="/task/1/sessions""#,
        r#"name="start""#,
        r#"name="end""#,
        r#"type="datetime-local""#,
        r#"name="message""#,
        "Add session",
        "/task/1?tab=sessions",
    ] {
        assert!(page.contains(wanted), "{wanted} in the page:\n{page}");
    }
    assert!(!page.contains("New note"), "only the one pop-up");
}
