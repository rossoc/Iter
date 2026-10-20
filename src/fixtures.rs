//! What the tests start from: an empty database, and the rows most of them
//! need, with the defaults a test does not care about.

use crate::db::Db;
use crate::models::{Board, Project, Settings};

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

/// [`project`] inserted into `db`: its id.
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
