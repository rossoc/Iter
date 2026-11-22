//! Shell completion callbacks, wired by `ArgValueCompleter` in [`crate::cli::args`]. They answer on a
//! keystroke with nowhere to report failure, so each queries exactly the strings it offers and treats
//! any error as no candidates. `args` names them as `ArgValueCompleter`s.

use crate::app::App;
use crate::db::Db;
use crate::error::Result;
use crate::models::{Board, Organization, Project, Tag, TaskStatus};
use clap_complete::engine::CompletionCandidate;

/// The candidates out of `names`. Every failure (non-UTF-8 input, unreadable database, empty
/// table) is no candidates.
fn candidates(
    current: &std::ffi::OsStr,
    names: impl FnOnce(&Db) -> Result<Vec<String>>,
) -> Vec<CompletionCandidate> {
    let Some(current) = current.to_str() else {
        return Vec::new();
    };
    let Ok(names) = App::open().and_then(|app| names(&app.db)) else {
        return Vec::new();
    };
    names
        .into_iter()
        .filter(|name| name.starts_with(current))
        .map(CompletionCandidate::new)
        .collect()
}

pub(crate) fn project_completer(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    candidates(current, Db::names::<Project>)
}

pub(crate) fn board_completer(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    candidates(current, Db::names::<Board>)
}

pub(crate) fn tag_completer(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    candidates(current, Db::names::<Tag>)
}

pub(crate) fn organization_completer(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    candidates(current, Db::names::<Organization>)
}

/// What completion offers for each task of `project` (any when `None`) at one of `statuses` (empty = all), in `<project>/<title>` order:
/// `<project>/<title>` where that names it alone, else its bare id (a repeated title, or one that
/// reads like a `#<id>` label, would resolve to the wrong task or to none).
pub(crate) fn offered(
    db: &Db,
    project: Option<&str>,
    statuses: &[TaskStatus],
) -> Result<Vec<String>> {
    let labels = db.task_labels_counted(project, statuses)?;
    Ok(labels
        .into_iter()
        .map(
            |(label, same_title)| match same_title == 1 && !label.title_reads_as_label() {
                true => format!("{}/{}", label.project, label.title),
                false => label.id.to_string(),
            },
        )
        .collect())
}

/// The id of every task: what `task pull/push --task` completes (the project is not known to the
/// completer, so a title could name several).
pub(crate) fn task_ids(db: &Db) -> Result<Vec<String>> {
    let labels = db.task_labels(None, &[])?;
    Ok(labels.iter().map(|label| label.id.to_string()).collect())
}

/// Dynamic completer for arguments naming a task as `<project>/<task>`,
/// offering only tasks at one of `statuses` -- or every task, when it's
/// empty.
fn task_completer_at(
    current: &std::ffi::OsStr,
    statuses: &'static [TaskStatus],
) -> Vec<CompletionCandidate> {
    candidates(current, |db| offered(db, None, statuses))
}

pub(crate) fn task_completer(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    task_completer_at(current, &[])
}

/// For `task pull/push --task`, a bare name: completion can't see the project positional, so it
/// offers every name.
pub(crate) fn task_name_completer(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    candidates(current, task_ids)
}

/// Dynamic completer for `session new`, which is only ever a sensible thing
/// to run on a task that hasn't been started: it refuses a task that already
/// has a session-config, and flips the one it does start to `wip`.
pub(crate) fn queued_task_completer(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    task_completer_at(current, &[TaskStatus::Queue])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::models::Task;

    fn insert_project(db: &Db, name: &str) -> i64 {
        db.insert(&fixtures::project(name))
            .expect("project inserts")
    }

    /// A task of status `wip`, as the tasks of these cases are.
    fn insert_task(db: &Db, project_id: i64, name: &str) -> i64 {
        let task = Task {
            status: TaskStatus::Wip,
            ..fixtures::task(project_id, name)
        };
        db.insert(&task).expect("task inserts")
    }

    #[test]
    fn offered_narrows_by_project_and_status_set() {
        let db = fixtures::db();
        let alpha = insert_project(&db, "alpha");
        let beta = insert_project(&db, "beta");
        insert_task(&db, alpha, "wip one"); // inserted as `wip`
        insert_task(&db, beta, "wip two");
        for (project, name, status) in [
            (alpha, "queued", TaskStatus::Queue),
            (alpha, "finished", TaskStatus::Done),
        ] {
            let task = Task {
                status,
                ..fixtures::task(project, name)
            };
            db.insert(&task).expect("task inserts");
        }

        assert_eq!(
            offered(&db, Some("alpha"), &[]).expect("refs"),
            vec!["alpha/finished", "alpha/queued", "alpha/wip one",]
        );
        assert_eq!(
            offered(&db, None, &[TaskStatus::Queue, TaskStatus::Wip]).expect("refs"),
            vec!["alpha/queued", "alpha/wip one", "beta/wip two",]
        );
        assert_eq!(
            offered(&db, Some("beta"), &[TaskStatus::Queue]).expect("refs"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn offered_pairs_every_task_with_its_project() {
        let db = fixtures::db();
        let beta = insert_project(&db, "beta");
        let alpha = insert_project(&db, "alpha");
        insert_task(&db, beta, "ship it");
        insert_task(&db, alpha, "write docs");
        insert_task(&db, alpha, "another");

        assert_eq!(
            offered(&db, None, &[]).expect("task refs"),
            vec!["alpha/another", "alpha/write docs", "beta/ship it",]
        );
    }

    /// The status filter must reach SQL, not run after loading every task.
    #[test]
    fn offered_can_narrow_to_one_status() {
        let db = fixtures::db();
        let project = insert_project(&db, "proj");
        insert_task(&db, project, "in progress"); // inserted as `wip`
        let queued = fixtures::task(project, "queued");
        db.insert(&queued).expect("task inserts");

        assert_eq!(
            offered(&db, None, &[TaskStatus::Queue]).expect("task refs"),
            vec!["proj/queued"]
        );
        assert_eq!(
            offered(&db, None, &[TaskStatus::Done]).expect("task refs"),
            Vec::<String>::new()
        );
    }

    /// R3: `task pull/push --task` completes the bare id of every task (the project is not known to
    /// the completer, so a title could name several); the help text, not the value, carries the title.
    #[test]
    fn task_names_are_the_ids_of_every_task() {
        let db = fixtures::db();
        let one = insert_project(&db, "one");
        let two = insert_project(&db, "two");
        insert_task(&db, one, "shared");
        insert_task(&db, two, "shared");
        insert_task(&db, one, "alone");

        let mut values = task_ids(&db).expect("task names");
        values.sort();
        assert_eq!(values, ["1", "2", "3"]);
    }

    /// R3: a task is offered as `<project>/<title>` when that names it alone, and as its bare id
    /// when the title repeats (a completion must never hand the shell a value that is ambiguous).
    #[test]
    fn completion_offers_a_repeated_title_by_its_id() {
        let db = fixtures::db();
        let web = insert_project(&db, "web");
        let api = insert_project(&db, "api");
        for (project, title) in [
            (web, "login"),
            (web, "login"),
            (web, "logout"),
            (api, "login"),
        ] {
            insert_task(&db, project, title);
        }
        assert_eq!(
            offered(&db, None, &[]).expect("refs"),
            ["api/login", "1", "2", "web/logout"]
        );
    }

    /// R3: a title that ends in `#<digits>` would be read back as an id, so it is offered by id.
    #[test]
    fn completion_offers_a_title_that_reads_like_a_label_by_its_id() {
        let db = fixtures::db();
        let web = insert_project(&db, "web");
        insert_task(&db, web, "fix #12");
        insert_task(&db, web, "plain");
        assert_eq!(offered(&db, None, &[]).expect("refs"), ["1", "web/plain"]);
    }
}
