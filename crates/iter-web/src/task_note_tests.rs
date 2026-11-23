//! A task's notes and sketches each on their own page (`/task/{id}/notes/{name}`,
//! `/task/{id}/sketches/{name}`), and the task's Notes tab as a table of links to them.
//!
//! Like `modal_tests`, each test runs again in a child process whose config folder and database
//! are a scratch folder, so the user's own are never read or written. The project has no notes
//! folder, so the task folder is the config folder's `tasks/1/`.

use super::toast_tests::{get, isolated_in, post};
use iter_core::{
    agent::taskdir::{self, TaskDir},
    config::config,
    db::Db,
    fixtures,
    models::{Project, Task},
};
use topcoat::router::StatusCode;

const NOTE: &str = "# Foo idea\nThe second line stays on the note's own page.\n";
const SKETCH: &str = "# Bar sketch\nA sketch body line only its page shows.\n";

/// Task 1 of project `app` in the scratch database, with `notes/foo.md` and `sketches/bar.md`
/// in its folder, a `plan.md` beside them and a hidden `notes/.secret.md`: the folder.
fn task_with_files() -> TaskDir {
    let db = Db::open(config().db_path().expect("a scratch database path")).expect("opens");
    let project = fixtures::add_project(&db, "app");
    assert_eq!(fixtures::add_task(&db, project, "Fix it"), 1);
    let project: Project = db.get(project).expect("reads").expect("the project");
    let task: Task = db.get(1).expect("reads").expect("the task");
    let dir = taskdir::of(&db, &project, &task).expect("the task folder");
    dir.ensure().expect("the folder is made");
    std::fs::write(dir.notes_dir().join("foo.md"), NOTE).expect("the note");
    std::fs::write(dir.notes_dir().join(".secret.md"), "hidden words").expect("hidden");
    std::fs::write(dir.notes_dir().join("other.txt"), "not markdown").expect("txt");
    std::fs::write(dir.sketches_dir().join("bar.md"), SKETCH).expect("the sketch");
    std::fs::write(dir.plan_path(), "# Plan words\n").expect("the plan");
    dir
}

fn ok(path: &str) -> String {
    let page = get(path);
    assert_eq!(page.status, StatusCode::OK, "{path}: {}", page.body);
    page.body
}

/// R1a: a task note opens on its own page, with the crumbs Project / Task / Notes / name
/// (no organization here) and the whole note rendered.
#[test]
fn a_task_note_has_its_own_page() {
    if isolated_in(module_path!(), "a_task_note_has_its_own_page", None) {
        return;
    }
    task_with_files();
    let page = ok("/task/1/notes/foo");
    assert!(page.contains("Foo idea"), "the note's heading: {page}");
    assert!(
        page.contains("The second line stays on the note"),
        "the whole note: {page}"
    );
    for crumb in [
        r#"href="/project/1?tab=tasks"#,
        r#"href="/task/1""#,
        r#"href="/task/1?tab=notes""#,
    ] {
        assert!(page.contains(crumb), "the crumb {crumb} in: {page}");
    }
    assert!(page.contains("foo"), "the name as the last crumb");
    assert!(!page.contains("<details"), "no drop-down: {page}");
}

/// R1a: a sketch opens on its own page too, read-only: no edit form, and a POST does not
/// change it.
#[test]
fn a_task_sketch_has_its_own_read_only_page() {
    if isolated_in(
        module_path!(),
        "a_task_sketch_has_its_own_read_only_page",
        None,
    ) {
        return;
    }
    let dir = task_with_files();
    let page = ok("/task/1/sketches/bar");
    assert!(page.contains("Bar sketch"), "{page}");
    assert!(
        page.contains("A sketch body line only its page shows."),
        "{page}"
    );
    assert!(page.contains(r#"href="/task/1?tab=notes""#), "{page}");

    let edit = get("/task/1/sketches/bar?tab=edit");
    assert!(
        !edit.body.contains(r#"name="text""#),
        "a sketch has no editor: {}",
        edit.body
    );
    let posted = post(
        "/task/1/sketches/bar",
        "text=changed",
        "navigate",
        Some("http://127.0.0.1:3000/task/1/sketches/bar"),
    );
    assert_ne!(posted.status, StatusCode::SEE_OTHER, "{}", posted.body);
    assert_eq!(
        std::fs::read_to_string(dir.sketches_dir().join("bar.md")).expect("still there"),
        SKETCH,
        "the sketch is unchanged"
    );
}

/// R1a: the note page has an Edit tab whose form posts the text back, and the save writes
/// the file and goes back to the note (303), as a daily note does.
#[test]
fn a_task_note_is_edited_and_saved() {
    if isolated_in(module_path!(), "a_task_note_is_edited_and_saved", None) {
        return;
    }
    let dir = task_with_files();
    let edit = ok("/task/1/notes/foo?tab=edit");
    assert!(
        edit.contains(r#"action="/task/1/notes/foo""#),
        "the form posts to the note: {edit}"
    );
    assert!(edit.contains(r#"name="text""#), "a text field: {edit}");
    assert!(edit.contains("The second line stays"), "the text to edit");

    let saved = post(
        "/task/1/notes/foo",
        "text=%23+Foo+idea%0D%0ANew+words.%0D%0A",
        "navigate",
        Some("http://127.0.0.1:3000/task/1/notes/foo?tab=edit"),
    );
    assert_eq!(saved.status, StatusCode::SEE_OTHER, "{}", saved.body);
    assert_eq!(
        std::fs::read_to_string(dir.notes_dir().join("foo.md")).expect("the note"),
        "# Foo idea\nNew words.\n"
    );
    assert!(ok("/task/1/notes/foo").contains("New words."));
}

/// R1c: only a name among `TaskDir::files` opens: an unknown name, a hidden or non-markdown
/// file, the plan beside the folders, and anything with `..` or `/` are a 404, for notes,
/// sketches and the note's save.
#[test]
fn only_the_folders_own_files_open() {
    if isolated_in(module_path!(), "only_the_folders_own_files_open", None) {
        return;
    }
    let dir = task_with_files();
    for path in [
        "/task/1/notes/nope",
        "/task/1/sketches/nope",
        "/task/1/notes/bar",
        "/task/1/sketches/foo",
        "/task/1/notes/.secret",
        "/task/1/notes/other",
        "/task/1/notes/other.txt",
        "/task/1/notes/foo.md",
        "/task/1/notes/plan",
        "/task/1/notes/%2E%2E",
        "/task/1/notes/..",
        "/task/1/notes/..%2Fplan",
        "/task/1/notes/..%2Fsketches%2Fbar",
        "/task/1/sketches/..%2Fnotes%2Ffoo",
        "/task/1/notes/%2E%2E%2F%2E%2E%2F%2E%2E%2Fiter.yaml",
        "/task/2/notes/foo",
    ] {
        let page = get(path);
        assert_eq!(page.status, StatusCode::NOT_FOUND, "{path}: {}", page.body);
        assert!(!page.body.contains("Plan words"), "{path} shows the plan");
        assert!(!page.body.contains("hidden words"), "{path} shows .secret");
    }
    let refused = post(
        "/task/1/notes/..%2Fplan",
        "text=overwritten",
        "cors",
        Some("http://127.0.0.1:3000/task/1"),
    );
    assert_eq!(refused.status, StatusCode::NOT_FOUND, "{}", refused.body);
    let created = post(
        "/task/1/notes/new-one",
        "text=made",
        "cors",
        Some("http://127.0.0.1:3000/task/1"),
    );
    assert_eq!(created.status, StatusCode::NOT_FOUND, "{}", created.body);
    assert_eq!(
        std::fs::read_to_string(dir.plan_path()).expect("the plan"),
        "# Plan words\n"
    );
    assert!(!dir.notes_dir().join("new-one.md").exists(), "no file made");
}

/// R1b: the Notes tab lists notes and sketches as rows linking to their pages, with the
/// first line as a preview; no `<details>` and not the whole text.
#[test]
fn the_notes_tab_is_a_table_of_links() {
    if isolated_in(module_path!(), "the_notes_tab_is_a_table_of_links", None) {
        return;
    }
    task_with_files();
    let page = ok("/task/1?tab=notes");
    assert!(page.contains(r#"href="/task/1/notes/foo""#), "{page}");
    assert!(page.contains(r#"href="/task/1/sketches/bar""#), "{page}");
    assert!(
        page.contains(r#"class="cover""#),
        "a cover link per row: {page}"
    );
    assert!(page.contains("Foo idea"), "the note's preview: {page}");
    assert!(page.contains("Bar sketch"), "the sketch's preview: {page}");
    assert!(!page.contains("<details"), "no drop-down: {page}");
    assert!(
        !page.contains("The second line stays") && !page.contains("A sketch body line"),
        "the full text is not inlined: {page}"
    );
    assert!(!page.contains("hidden words"));
}

/// R1b: the `.files details` rules are gone from the stylesheet.
#[test]
fn the_stylesheet_has_no_files_drop_down() {
    if isolated_in(
        module_path!(),
        "the_stylesheet_has_no_files_drop_down",
        None,
    ) {
        return;
    }
    let css = get("/style.css").body;
    assert!(!css.contains(".files details"), "{css}");
}
