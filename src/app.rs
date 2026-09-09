//! The state a single `iter` run is carried out against.
//!
//! Two things, both of which every command wants and neither of which it
//! should fetch for itself:
//!
//! * the database — opened once, at the top, instead of by each command
//!   body in turn;
//! * `now` — taken once, so every duration and every report in one run
//!   measures against the same instant. Read per command, a long `task
//!   push` followed by a report could straddle a minute boundary and
//!   disagree with itself.
//!
//! This is ergonomics and consistency, not speed: exactly one database open
//! happens per process either way.
//!
//! Deliberately no `config` field. [`crate::config::config`] already hands
//! out a `&'static Config` from a `OnceLock`, reachable from anywhere; a
//! field aliasing it would only make `App` look like it owns something it
//! doesn't.
//!
//! Note that shell completion does **not** run against an `App` — see
//! [`crate::completion`] for why it must not.

use crate::db::{Db, open_db};
use chrono::{Local, NaiveDateTime};

pub struct App {
    pub db: Db,
    /// One instant for the whole run.
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
