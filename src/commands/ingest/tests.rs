use super::*;
use crate::fixtures;
use crate::models::TaskStatus;

fn world(name: &str) -> (App, Project, Task, PathBuf) {
    let notes = fixtures::scratch(&format!("ingestcmd-{name}"));
    let db = fixtures::db();
    let mut p = fixtures::project("app");
    p.notes_path = notes.display().to_string();
    let project_id = db.insert(&p).unwrap();
    let task_id = db
        .insert(&Task {
            status: TaskStatus::Done,
            ..fixtures::task(project_id, "Fix it")
        })
        .unwrap();
    let project = db.get::<Project>(project_id).unwrap().unwrap();
    let task = db.get::<Task>(task_id).unwrap().unwrap();
    (App::with_db(db), project, task, notes)
}

#[test]
fn a_finished_tasks_notes_go_into_todays_note_once() {
    let (app, project, task, notes) = world("bridge");
    // no notes folder of the task, nothing to add
    assert_eq!(bridge_task_notes(&app, &project, &task).unwrap(), 0);
    let dir = crate::agent::taskdir::of(&app.db, &project, &task).unwrap();
    dir.ensure().unwrap();
    std::fs::write(dir.notes_dir().join("thought.md"), "Maybe per route.\n").unwrap();
    std::fs::write(dir.notes_dir().join("empty.md"), "  \n").unwrap();
    assert_eq!(bridge_task_notes(&app, &project, &task).unwrap(), 1);
    let today = chrono::Local::now().date_naive();
    let text = std::fs::read_to_string(notes.join(format!("{}.md", today.format("%Y-%m-%d"))))
        .unwrap();
    assert!(text.contains("## Task: Fix it\n\n### thought\n\nMaybe per route."));
    assert!(!text.contains("empty"));
    // done again: the section is there already
    assert_eq!(bridge_task_notes(&app, &project, &task).unwrap(), 0);
}

fn git_in(dir: &std::path::Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(status.status.success(), "git {args:?}: {status:?}");
}

/// An organization repository `root` whose project `app` keeps its notes in
/// `root/app` -- the usual layout -- with one committed page.
fn repo(name: &str) -> (Db, Project, PathBuf) {
    let root = fixtures::scratch(&format!("pending-{name}"));
    let notes = root.join("app");
    std::fs::create_dir_all(notes.join("wiki/ideas")).unwrap();
    std::fs::write(
        notes.join("wiki/ideas/b.md"),
        "---\ndescription: b\ntags: [x]\n---\nold\n",
    )
    .unwrap();
    git_in(&root, &["init", "-q", "-b", "main"]);
    git_in(&root, &["config", "user.email", "t@t"]);
    git_in(&root, &["config", "user.name", "t"]);
    git_in(&root, &["add", "-A"]);
    git_in(&root, &["commit", "-q", "-m", "init"]);
    let db = fixtures::db();
    let mut p = fixtures::project("app");
    p.notes_path = notes.display().to_string();
    let id = db.insert(&p).unwrap();
    let project = db.get::<Project>(id).unwrap().unwrap();
    (db, project, notes)
}

#[test]
fn pending_pages_are_found_with_their_diffs_in_a_repository_above_the_notes() {
    let (db, project, notes) = repo("nested");
    std::fs::write(
        notes.join("wiki/ideas/a.md"),
        "---\ndescription: a\ntags: [x]\n---\nnew\n",
    )
    .unwrap();
    std::fs::write(
        notes.join("wiki/ideas/b.md"),
        "---\ndescription: b\ntags: [x]\n---\nnewer\n",
    )
    .unwrap();
    // the user's own files in the same folder are not the agent's
    std::fs::write(notes.join("wiki/settings.yaml"), "ingest: manual\n").unwrap();
    let review = review(&db, &project).unwrap();
    let seen: Vec<(&str, &str)> = review
        .changes
        .iter()
        .map(|c| (c.path.as_str(), c.kind))
        .collect();
    assert_eq!(
        seen,
        [("wiki/ideas/a.md", "new"), ("wiki/ideas/b.md", "changed")]
    );
    assert!(review.changes[0].diff.contains("+new"));
    assert!(
        review.changes[1].diff.contains("-old") && review.changes[1].diff.contains("+newer")
    );
}

#[test]
fn throwing_away_touches_the_pages_and_nothing_else() {
    let (db, project, notes) = repo("revert");
    std::fs::write(
        notes.join("wiki/ideas/a.md"),
        "---\ndescription: a\n---\nnew\n",
    )
    .unwrap();
    std::fs::write(notes.join("wiki/ideas/b.md"), "changed\n").unwrap();
    std::fs::write(notes.join("wiki/settings.yaml"), "ingest: manual\n").unwrap();
    // a commit that failed leaves the page staged
    git_in(&notes, &["add", "wiki/ideas/a.md"]);
    let paths = settle(&db, &project, false).unwrap();
    assert_eq!(paths, ["wiki/ideas/a.md", "wiki/ideas/b.md"]);
    assert!(!notes.join("wiki/ideas/a.md").exists());
    assert!(
        std::fs::read_to_string(notes.join("wiki/ideas/b.md"))
            .unwrap()
            .contains("old")
    );
    assert!(notes.join("wiki/settings.yaml").exists());
    assert!(review(&db, &project).unwrap().changes.is_empty());
}

#[test]
fn accepting_commits_the_pages_only() {
    let (db, project, notes) = repo("accept");
    std::fs::write(
        notes.join("wiki/ideas/a.md"),
        "---\ndescription: a\n---\nnew\n",
    )
    .unwrap();
    std::fs::write(notes.join("wiki/settings.yaml"), "ingest: manual\n").unwrap();
    settle(&db, &project, true).unwrap();
    let committed = process::output(
        "git",
        Some(&notes.display().to_string()),
        &["show", "--name-only", "--format=", "HEAD"],
    )
    .unwrap();
    assert!(committed.contains("wiki/ideas/a.md") && !committed.contains("settings.yaml"));
    assert!(review(&db, &project).unwrap().changes.is_empty());
}

#[test]
fn only_one_agent_holds_the_wikis_lock() {
    let (db, project, _) = repo("lock");
    let target = Target::new(&db, project).unwrap();
    let first = lock(&target).unwrap();
    assert!(busy(&target) && lock(&target).is_err());
    drop(first);
    assert!(!busy(&target) && lock(&target).is_ok());
}

#[test]
fn an_ingest_needs_a_project_with_a_notes_folder() {
    let db = fixtures::db();
    let id = db.insert(&fixtures::project("bare")).unwrap();
    let project = db.get::<Project>(id).unwrap().unwrap();
    assert!(Target::new(&db, project).is_err());
}
