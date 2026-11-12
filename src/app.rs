//! The state one `iter` run is carried out against: the database (opened once) and `now` (taken
//! once, so every duration and report in a run measures against the same instant). Not carried:
//! the config, which stays the process-wide [`crate::config::config`] that about a dozen modules
//! read directly (moving it onto `App` is a larger change, deliberately not made yet). Shell
//! completion does not run against an `App` (see [`crate::completion`]).

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
        App::over(open_db(), Local::now().naive_local())
    }

    /// An app over a database the caller already has, at an instant it chose: what a web request
    /// builds (it opens its own database) and what the other constructors reduce to.
    pub fn over(db: Db, now: NaiveDateTime) -> Self {
        App { db, now }
    }

    /// An app over an explicit database — what a test uses to drive
    /// commands without a config file or an `$XDG_CONFIG_HOME` to arrange.
    #[cfg(test)]
    pub fn with_db(db: Db) -> Self {
        App::over(db, Local::now().naive_local())
    }
}
