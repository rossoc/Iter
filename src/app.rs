//! The state one `iter` run is carried out against: the database (opened once) and `now` (taken
//! once, so every duration and report in a run measures against the same instant). No `config`
//! field: [`crate::config::config`] is already global. Shell completion does not run against an
//! `App` (see [`crate::completion`]).

use crate::db::{Db, open_db};
use chrono::{Local, NaiveDateTime};

pub struct App {
    pub db: Db,
    pub now: NaiveDateTime,
}

impl App {
    /// Opens the configured database, or exits saying why (see
    /// [`open_db`]), and stamps the run's `now`.
    pub fn new() -> Self {
        App {
            db: open_db(),
            now: Local::now().naive_local(),
        }
    }

    /// An app over an explicit database — what a test uses to drive
    /// commands without a config file or an `$XDG_CONFIG_HOME` to arrange.
    #[cfg(test)]
    pub fn with_db(db: Db) -> Self {
        App {
            db,
            now: Local::now().naive_local(),
        }
    }
}
