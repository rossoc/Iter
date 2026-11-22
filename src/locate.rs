//! Addressing: the project or task a [`TaskRef`], the typed text, or "here" names. From
//! `utils::resolve` in isolation step 4; the whole module is a stub until then.

use crate::db::Db;
use crate::error::Result;
use crate::models::{Project, Task, TaskRef};

/// The one resolver: the project and task a reference names. An id (`42`, `#42`) always
/// resolves; `<project>/<title>` resolves when the title is unique in the project, else
/// `AmbiguousTask`; in a label (`web/old title #42`) the id wins over a stale title.
// stub: I3 (`utils::resolve::resolve_task` still does the work)
#[allow(dead_code)]
pub fn task(db: &Db, task_ref: &TaskRef) -> Result<(Project, Task)> {
    let _ = (db, task_ref);
    unimplemented!("stub: I3")
}

/// A task of `project` by what `--task` of pull and push says: an id (`42`, `#42`) when a task
/// of *that* project has it, otherwise a title, which must be unique.
// stub: I3 (`utils::resolve::find_task_or_err` still does the work)
#[allow(dead_code)]
pub fn task_in(db: &Db, project: &Project, text: &str) -> Result<Task> {
    let _ = (db, project, text);
    unimplemented!("stub: I3")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::IterError;
    use crate::fixtures;
    use crate::models::Table;

    /// Project `web` with `login` (1), `login` (2), `logout` (3); project `api` with `login` (4)
    /// and `2024` (5).
    fn world() -> Db {
        let db = fixtures::db();
        let web = db.insert(&fixtures::project("web")).unwrap();
        let api = db.insert(&fixtures::project("api")).unwrap();
        for (project, title) in [
            (web, "login"),
            (web, "login"),
            (web, "logout"),
            (api, "login"),
            (api, "2024"),
        ] {
            fixtures::add_task(&db, project, title);
        }
        db
    }

    fn by_ref(db: &Db, text: &str) -> Result<i64> {
        task(db, &text.parse::<TaskRef>()?).map(|(_, task)| task.id())
    }

    fn in_web(db: &Db, text: &str) -> Result<i64> {
        let web = db.resolve::<Project>("web")?;
        task_in(db, &web, text).map(|task| task.id())
    }

    /// R2, R3: what `locate::task` resolves, one row per form of reference.
    #[test]
    fn a_reference_resolves_to_its_task_by_id_title_or_label() {
        let db = world();
        for (text, id) in [
            ("3", 3),
            ("#3", 3),
            ("web/logout", 3),
            ("api/login", 4),
            ("web/logout #3", 3),
            ("web/a stale title #3", 3),
            ("web/login #2", 2),
        ] {
            assert_eq!(by_ref(&db, text).unwrap(), id, "{text}");
        }
    }

    #[test]
    fn a_reference_that_names_no_task_is_refused_with_its_own_error() {
        let db = world();
        for text in ["#99", "99"] {
            assert!(
                matches!(by_ref(&db, text), Err(IterError::NotFound { .. })),
                "{text}"
            );
        }
        for text in [
            "web/missing",
            "api/logout",
            "web/logout #4",
            "web/login #99",
        ] {
            assert!(
                matches!(by_ref(&db, text), Err(IterError::TaskNotFound { .. })),
                "{text}"
            );
        }
        assert!(matches!(
            by_ref(&db, "nosuch/login"),
            Err(IterError::NotFound { .. })
        ));
    }

    #[test]
    fn a_repeated_title_is_ambiguous_and_lists_every_id() {
        let db = world();
        match by_ref(&db, "web/login") {
            Err(IterError::AmbiguousTask {
                project,
                title,
                ids,
            }) => {
                assert_eq!(
                    (project.as_str(), title.as_str(), ids),
                    ("web", "login", vec![1, 2])
                );
            }
            other => panic!("expected AmbiguousTask, got {other:?}"),
        }
    }

    /// R2, R4: `--task`: an id of the project, else a title; another project's id is not found.
    #[test]
    fn a_task_of_the_project_is_found_by_id_then_by_title() {
        let db = world();
        for (text, id) in [("3", 3), ("#3", 3), ("logout", 3), ("#1", 1)] {
            assert_eq!(in_web(&db, text).unwrap(), id, "{text}");
        }
        for text in ["4", "#4", "missing"] {
            assert!(
                matches!(in_web(&db, text), Err(IterError::TaskNotFound { .. })),
                "{text}: task 4 is in `api`, and `pull web --task 4` never acts on it"
            );
        }
        assert!(matches!(
            in_web(&db, "login"),
            Err(IterError::AmbiguousTask { .. })
        ));
    }

    /// With no task 2024 in the project, `2024` is the title of one (`api` has task 5, not `web`).
    #[test]
    fn an_all_digit_title_is_a_title_when_no_task_has_that_id() {
        let db = world();
        let web = db.resolve::<Project>("web").unwrap().id();
        let numeric = fixtures::add_task(&db, web, "2024");
        assert_eq!(in_web(&db, "2024").unwrap(), numeric);
    }
}
