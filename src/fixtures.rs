//! What the tests start from: an empty database, and the rows most of them
//! need, with the defaults a test does not care about.

use std::path::PathBuf;

use chrono::{NaiveDate, NaiveDateTime};

use crate::db::Db;
use crate::models::{Board, Project, Session, Settings, Task, TaskStatus};

/// A fresh, empty, migrated database held entirely in memory.
pub fn db() -> Db {
    Db::open(":memory:").expect("in-memory database opens")
}

/// A project named `name` with the default settings, in `/tmp/{name}`, in
/// no organization and on no board; not inserted.
pub fn project(name: &str) -> Project {
    let mut p = Project::template(&Settings::default());
    p.name = name.into();
    p.base_path = format!("/tmp/{name}");
    p
}

/// [`project`] inserted into `db`: its id. (Only the web's tests use it.)
#[cfg_attr(not(feature = "web"), allow(dead_code))]
pub fn add_project(db: &Db, name: &str) -> i64 {
    db.insert(&project(name)).expect("project inserts")
}

/// A board named `name` with no description; not inserted.
pub fn board(name: &str) -> Board {
    Board {
        name: name.into(),
        ..Board::template()
    }
}

/// A queued task named `name` under project `project_id`, with nothing else
/// set; not inserted. Override with struct update:
/// `Task { status: TaskStatus::Done, ..fixtures::task(p, "x") }`.
pub fn task(project_id: i64, name: &str) -> Task {
    Task {
        name: name.into(),
        ..Task::template(project_id, String::new(), TaskStatus::Queue)
    }
}

/// [`task`] inserted into `db`: its id.
pub fn add_task(db: &Db, project_id: i64, name: &str) -> i64 {
    db.insert(&task(project_id, name)).expect("task inserts")
}

/// A session of `task_id` from `start` to `end` (open when `None`), with no
/// message; not inserted.
pub fn session(task_id: i64, start: NaiveDateTime, end: Option<NaiveDateTime>) -> Session {
    Session {
        id: None,
        task_id,
        start,
        end,
        message: None,
    }
}

/// [`session`] inserted into `db`: its id.
pub fn add_session(db: &Db, task_id: i64, start: NaiveDateTime, end: Option<NaiveDateTime>) -> i64 {
    db.insert(&session(task_id, start, end))
        .expect("session inserts")
}

/// `"2026-09-04 09:00"` as a timestamp; panics naming the bad fixture.
pub fn dt(s: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M")
        .unwrap_or_else(|e| panic!("bad fixture datetime {s:?}: {e}"))
}

/// `"2026-09-04"` as a date; panics naming the bad fixture.
pub fn day(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .unwrap_or_else(|e| panic!("bad fixture date {s:?}: {e}"))
}

/// A fresh scratch directory path, unique to this process and to `tag`
/// (keep each test's tag distinct so parallel tests never share one):
/// removed if it was there, not created.
pub fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("iter-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// What the agent tests start from: the project `app` keeping its notes in a
/// scratch folder, with one queued task `Fix it` whose description is
/// "Retries hammer the api.".
#[cfg_attr(not(feature = "web"), allow(dead_code))]
pub struct World {
    pub db: Db,
    pub project_id: i64,
    pub task_id: i64,
    /// The project's notes folder: [`scratch`]`(tag)`, not created.
    pub notes: PathBuf,
}

pub fn world(tag: &str) -> World {
    let notes = scratch(tag);
    let db = db();
    let project_id = db
        .insert(&Project {
            notes_path: notes.display().to_string(),
            ..project("app")
        })
        .expect("project inserts");
    let task = Task {
        description: "Retries hammer the api.".into(),
        ..task(project_id, "Fix it")
    };
    let task_id = db.insert(&task).expect("task inserts");
    World {
        db,
        project_id,
        task_id,
        notes,
    }
}
