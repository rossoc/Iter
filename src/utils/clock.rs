//! The session clock: opening and closing the stretches of time a task is
//! worked in.
//!
//! Deliberately not on `Db`: these encode policy -- don't double-start,
//! carry a detach message onto the session it belongs to, only the last
//! client out stops the clock -- rather than storage. Shared by `iter
//! session start/stop`, by `iter task done`, and by the tmux hooks, which
//! is why they answer to none of those modules.

use crate::db::{Db, Table};
use crate::error::Result;
use crate::models::Session;
use crate::tmux;
use chrono::{Local, NaiveDateTime};

pub(crate) fn start_session(db: &Db, task_id: i64) -> Result<()> {
    if db.open_session_for_task(task_id)?.is_some() {
        return Ok(()); // already has an open session; don't double-start
    }
    let session = Session {
        id: None,
        task_id,
        start: Local::now().naive_local(),
        end: None,
        message: None,
    };
    db.insert(&session)?;
    Ok(())
}

/// What closing a task's clock did.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Closed {
    /// The task had no open session.
    Nothing,
    /// The session was closed and kept.
    Kept,
    /// The session ran under [`Session::MIN_SECONDS`], so it was deleted
    /// rather than closed.
    Discarded,
}

pub(crate) fn close_open_session(db: &Db, task_id: i64, message: Option<&str>) -> Result<Closed> {
    close_open_session_at(db, task_id, message, Local::now().naive_local())
}

fn close_open_session_at(
    db: &Db,
    task_id: i64,
    message: Option<&str>,
    now: NaiveDateTime,
) -> Result<Closed> {
    let Some(mut open) = db.open_session_for_task(task_id)? else {
        return Ok(Closed::Nothing);
    };
    debug_assert!(
        open.is_ongoing(),
        "open_session_for_task returned a closed session"
    );
    if open.too_short(now) {
        // A blip, not work: never stored, so no report has to filter it.
        db.delete::<Session>(open.id())?;
        return Ok(Closed::Discarded);
    }
    open.end = Some(now);
    if let Some(m) = message {
        open.message = Some(m.to_string());
    }
    db.update(open.id(), &open)?;
    Ok(Closed::Kept)
}

/// Starts the session of whatever task `tmux_session` belongs to. A name
/// `iter` doesn't know is not an error: these hooks are global, so they
/// fire for every tmux session on the server, not just ours.
pub(crate) fn start_for_tmux_session(db: &Db, tmux_session: &str) -> Result<()> {
    match db.find_session_config_by_tmux_name(tmux_session)? {
        Some(session_config) => start_session(db, session_config.task_id),
        None => Ok(()),
    }
}

/// Closes the open session of whatever task `tmux_session` belongs to,
/// carrying over a message left in the tmux option by whatever detached
/// (see `tmux::DETACH_MESSAGE_OPTION`) so a note typed on the way out lands
/// on the session it belongs to.
///
/// A tmux session can have several clients on it -- a second terminal, or
/// someone pairing -- and it's still being worked as long as one of them is
/// there, so only the last one out stops the clock. The client that fired
/// this hook is already off tmux's list by now, so what's left on it is
/// exactly what "still being worked" means.
pub(crate) fn stop_for_tmux_session(db: &Db, tmux_session: &str) -> Result<()> {
    let Some(session_config) = db.find_session_config_by_tmux_name(tmux_session)? else {
        return Ok(()); // not one of ours -- ignore
    };
    if tmux::any_client_attached(tmux_session) {
        return Ok(()); // somebody else is still in there
    }
    let message = tmux::take_detach_message(tmux_session);
    close_open_session(db, session_config.task_id, message.as_deref())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Project, Task, TaskStatus};
    use chrono::NaiveDate;

    fn at(h: u32, m: u32, s: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 9, 1)
            .expect("valid date")
            .and_hms_opt(h, m, s)
            .expect("valid time")
    }

    /// An in-memory database holding one task with a session open since
    /// 09:00:00. Returns the database and the task's id.
    fn open_at_nine() -> (Db, i64) {
        let db = Db::open(":memory:").expect("in-memory database opens");
        let project_id = db
            .insert(&Project {
                id: None,
                organization_id: None,
                board_id: None,
                name: "proj".to_string(),
                description: String::new(),
                base_path: "/tmp/proj".to_string(),
                github: false,
                tmux: false,
                auto_branch: false,
                branch_template: "feat/{task}".to_string(),
                default_branch: "main".to_string(),
                github_project: String::new(),
            })
            .expect("project inserts");
        let task_id = db
            .insert(&Task {
                id: None,
                project_id,
                name: "a task".to_string(),
                description: String::new(),
                github_issue: None,
                status: TaskStatus::Wip,
                branch_prefix: String::new(),
                matrix_placed: false,
                start_time: None,
                duration: None,
            })
            .expect("task inserts");
        db.insert(&Session {
            id: None,
            task_id,
            start: at(9, 0, 0),
            end: None,
            message: None,
        })
        .expect("session inserts");
        (db, task_id)
    }

    #[test]
    fn a_session_under_a_minute_is_deleted_not_closed() {
        let (db, task_id) = open_at_nine();
        let closed = close_open_session_at(&db, task_id, Some("quick look"), at(9, 0, 59))
            .expect("closing succeeds");
        assert_eq!(closed, Closed::Discarded);
        assert!(db.sessions_for_task(task_id).expect("lookup").is_empty());
    }

    #[test]
    fn a_session_of_exactly_a_minute_is_kept() {
        let (db, task_id) = open_at_nine();
        let closed = close_open_session_at(&db, task_id, Some("done"), at(9, 1, 0))
            .expect("closing succeeds");
        assert_eq!(closed, Closed::Kept);
        let sessions = db.sessions_for_task(task_id).expect("lookup");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].end, Some(at(9, 1, 0)));
        assert_eq!(sessions[0].message.as_deref(), Some("done"));
    }

    #[test]
    fn closing_with_nothing_open_changes_nothing() {
        let (db, task_id) = open_at_nine();
        close_open_session_at(&db, task_id, None, at(10, 0, 0)).expect("first close");
        let again = close_open_session_at(&db, task_id, None, at(11, 0, 0)).expect("second close");
        assert_eq!(again, Closed::Nothing);
        assert_eq!(db.sessions_for_task(task_id).expect("lookup").len(), 1);
    }
}
