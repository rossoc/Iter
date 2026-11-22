//! The state one `iter` run is carried out against: the database (opened once) and `now` (taken
//! once, so every duration and report in a run measures against the same instant). Not carried:
//! the config, which stays the process-wide [`crate::config::config`] that [`App::open`] loads.

use crate::config;
use crate::db::Db;
use crate::error::{IterError, Result};
use chrono::{Local, NaiveDateTime};

pub struct App {
    pub db: Db,
    pub now: NaiveDateTime,
}

impl App {
    /// Loads the config, opens the database it names (made beside the config file when it names
    /// none) and stamps the run's `now`. A database that will not open is
    /// [`IterError::OpenDb`], naming its path.
    pub fn open() -> Result<Self> {
        let path = config::load()?.db_path()?;
        let db = Db::open(&path).map_err(|source| IterError::OpenDb {
            path,
            source: Box::new(source),
        })?;
        Ok(App::over(db, Local::now().naive_local()))
    }

    /// An app over a database the caller already has, at an instant it chose: what a web request
    /// builds (it opens its own database) and what the other constructors reduce to.
    pub fn over(db: Db, now: NaiveDateTime) -> Self {
        App { db, now }
    }

    /// An app over an explicit database — what a test uses to drive
    /// commands without a config file or an `$XDG_CONFIG_HOME` to arrange.
    #[cfg(any(test, feature = "test-support"))]
    pub fn with_db(db: Db) -> Self {
        App::over(db, Local::now().naive_local())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::IterError;
    use crate::fixtures::in_scratch_config as isolated;

    /// Round I3 (step 3): opening is a `Result`. With no `db_path` the database is made beside the
    /// config file and `now` is the instant of the call.
    #[test]
    fn open_makes_the_database_beside_the_config_and_stamps_now() {
        if isolated(
            module_path!(),
            "open_makes_the_database_beside_the_config_and_stamps_now",
            None,
        ) {
            return;
        }
        let before = Local::now().naive_local();
        let app = App::open().expect("opens");
        let after = Local::now().naive_local();
        assert!(before <= app.now && app.now <= after);
        let db = crate::config::config_dir().unwrap().join("iter.db");
        assert!(db.exists(), "{}", db.display());
    }

    /// The error keeps the path, so `main` can say which database failed; the process lives on.
    #[test]
    fn a_database_that_will_not_open_is_an_error_naming_its_path() {
        let name = "a_database_that_will_not_open_is_an_error_naming_its_path";
        if isolated(module_path!(), name, Some("db_path: /dev/null/iter.db\n")) {
            return;
        }
        match App::open() {
            Err(error @ IterError::OpenDb { .. }) => {
                let IterError::OpenDb { path, .. } = &error else {
                    unreachable!()
                };
                assert_eq!(path, std::path::Path::new("/dev/null/iter.db"));
                assert!(
                    error
                        .to_string()
                        .starts_with("failed to open database at /dev/null/iter.db: "),
                    "{error}"
                );
            }
            Err(other) => panic!("expected OpenDb, got {other:?}"),
            Ok(_) => panic!("opened a database under /dev/null"),
        }
    }

    /// A typo in the config is an error to report, not an exit, and not an `OpenDb`.
    #[test]
    fn an_invalid_config_is_an_error_not_an_exit() {
        let name = "an_invalid_config_is_an_error_not_an_exit";
        if isolated(module_path!(), name, Some("pause_gap: 5\n")) {
            return;
        }
        assert!(matches!(App::open(), Err(IterError::InvalidConfig { .. })));
    }
}
