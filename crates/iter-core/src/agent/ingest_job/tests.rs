use super::*;
use crate::fixtures::{self, git_in, last_subject, wiki_repo as repo};
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
    assert_eq!(bridge_task_notes(&app, &project, &task).unwrap(), 0);
    let dir = crate::agent::taskdir::of(&app.db, &project, &task).unwrap();
    dir.ensure().unwrap();
    std::fs::write(dir.notes_dir().join("thought.md"), "Maybe per route.\n").unwrap();
    std::fs::write(dir.notes_dir().join("empty.md"), "  \n").unwrap();
    assert_eq!(bridge_task_notes(&app, &project, &task).unwrap(), 1);
    let today = chrono::Local::now().date_naive();
    let text =
        std::fs::read_to_string(notes.join(format!("{}.md", today.format("%Y-%m-%d")))).unwrap();
    // the heading may carry the task's id after the name (R7 of task 303)
    let heading = text
        .lines()
        .find(|l| l.starts_with("## Task: Fix it"))
        .unwrap_or_else(|| panic!("no section of the task: {text}"));
    assert!(
        text.contains(&format!("{heading}\n\n### thought\n\nMaybe per route.")),
        "{text}"
    );
    assert!(!text.contains("empty"));
    assert_eq!(bridge_task_notes(&app, &project, &task).unwrap(), 0);
}

/// R7 of task 303 (#242): two tasks of a project with the same name, both done on the same day,
/// each keep their own section in today's note. Today the second is skipped, because the
/// heading (`## Task: <name>`) is already there. Bridging either again still adds nothing.
#[test]
fn two_same_name_tasks_done_on_one_day_keep_both_sections() {
    let (app, project, first, notes) = world("bridge-twins");
    let second_id = app
        .db
        .insert(&Task {
            status: TaskStatus::Done,
            ..fixtures::task(project.id(), "Fix it")
        })
        .unwrap();
    let second = app.db.get::<Task>(second_id).unwrap().unwrap();
    for (task, words) in [(&first, "Retry per route."), (&second, "Cache the token.")] {
        let dir = crate::agent::taskdir::of(&app.db, &project, task).unwrap();
        dir.ensure().unwrap();
        std::fs::write(dir.notes_dir().join("thought.md"), format!("{words}\n")).unwrap();
    }
    assert_eq!(bridge_task_notes(&app, &project, &first).unwrap(), 1);
    assert_eq!(
        bridge_task_notes(&app, &project, &second).unwrap(),
        1,
        "the second task's notes were skipped"
    );
    let today = chrono::Local::now().date_naive();
    let text =
        std::fs::read_to_string(notes.join(format!("{}.md", today.format("%Y-%m-%d")))).unwrap();
    assert!(
        text.contains("Retry per route.") && text.contains("Cache the token."),
        "{text}"
    );
    let headings: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with("## Task: Fix it"))
        .collect();
    assert_eq!(headings.len(), 2, "{text}");
    assert_ne!(headings[0], headings[1], "{text}");
    for id in [first.id(), second_id] {
        assert!(
            headings.iter().any(|h| h.contains(&id.to_string())),
            "no heading names task {id}: {headings:?}"
        );
    }
    // still once each
    assert_eq!(bridge_task_notes(&app, &project, &first).unwrap(), 0);
    assert_eq!(bridge_task_notes(&app, &project, &second).unwrap(), 0);
    let _ = std::fs::remove_dir_all(&notes);
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
    assert!(review.changes[1].diff.contains("-old") && review.changes[1].diff.contains("+newer"));
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
    assert_eq!(last_subject(&notes), "wiki: ingest");
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

/// Characterisation: the ingest's stamp is the local time as `YYYY-MM-DDTHH:MM:SS`
/// (it goes into file names with its colons removed).
#[test]
fn the_ingest_is_stamped_with_the_local_time_and_a_t() {
    let stamp = now();
    assert_eq!(stamp.as_bytes()[10], b'T', "{stamp}");
    let at = chrono::NaiveDateTime::parse_from_str(&stamp, "%Y-%m-%dT%H:%M:%S").unwrap();
    let off = (chrono::Local::now().naive_local() - at)
        .num_seconds()
        .abs();
    assert!(off < 5, "{stamp} is {off}s from now");
}
