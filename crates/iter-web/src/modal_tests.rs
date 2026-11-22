//! What the two task pop-ups (`pages/task/task_notes.rs`, `pages/task/task_sessions.rs`, both `form_modal`) put on
//! the page: the modal's id and title, the form's action, its fields, the submit label and the way back.
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

/// Runs the test `name` isolated, then checks the pop-up of `tab`/`new` holds every `wanted` text and not `other`.
fn pop_up(name: &str, tab: &str, new: &str, wanted: &[&str], other: &str) {
    if isolated_in(module_path!(), name, None) {
        return;
    }
    let page = modal_page(tab, new);
    for wanted in wanted {
        assert!(page.contains(wanted), "{wanted} in the page:\n{page}");
    }
    assert!(!page.contains(other), "only the one pop-up");
}

#[test]
fn the_note_pop_up_asks_for_a_name_and_a_text() {
    pop_up(
        "the_note_pop_up_asks_for_a_name_and_a_text",
        "notes",
        "note",
        &[
            r#"id="note-title""#,
            "New note",
            r#"action="/task/1/notes""#,
            r#"name="name""#,
            r#"name="text""#,
            "Add note",
            "/task/1?tab=notes",
        ],
        "New session",
    );
}

#[test]
fn the_session_pop_up_asks_for_two_moments_and_a_note() {
    pop_up(
        "the_session_pop_up_asks_for_two_moments_and_a_note",
        "sessions",
        "session",
        &[
            r#"id="session-title""#,
            "New session",
            r#"action="/task/1/sessions""#,
            r#"name="start""#,
            r#"name="end""#,
            r#"type="datetime-local""#,
            r#"name="message""#,
            "Add session",
            "/task/1?tab=sessions",
        ],
        "New note",
    );
}
