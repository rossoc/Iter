//! The session clock: opening and closing the stretches a task is worked in. Not on `Db`: it
//! encodes policy (no double-start, a detach message follows its session, only the last client out
//! stops the clock), shared by session start/stop, `task done` and the tmux hooks.

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

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Closed {
    Nothing,
    Kept,
    /// The session ran under [`Session::MIN_SECONDS`] and was deleted, not closed.
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

/// Closes `task_id`'s open session if a previous tmux server left it open (started before
/// `server_start`): a reboot closes nothing, so the clock would run through the downtime. Closed at
/// `last_save` (tmux-resurrect) when that falls between the two, else the end is unknown and the
/// session is deleted. A session of this server is left alone (`Closed::Nothing`), so rerunning is
/// harmless.
pub(crate) fn close_stale_session(
    db: &Db,
    task_id: i64,
    server_start: NaiveDateTime,
    last_save: Option<NaiveDateTime>,
) -> Result<Closed> {
    let Some(open) = db.open_session_for_task(task_id)? else {
        return Ok(Closed::Nothing);
    };
    if open.start >= server_start {
        return Ok(Closed::Nothing); // this server's own clock
    }
    match last_save.filter(|&save| open.start < save && save < server_start) {
        Some(end) => close_open_session_at(db, task_id, None, end),
        None => {
            db.delete::<Session>(open.id())?;
            Ok(Closed::Discarded)
        }
    }
}

/// Starts the session of the task `tmux_session` belongs to. An unknown name is not an error: the
/// hooks are global and fire for every tmux session.
pub(crate) fn start_for_tmux_session(db: &Db, tmux_session: &str) -> Result<()> {
    match db.find_session_config_by_tmux_name(tmux_session)? {
        Some(session_config) => start_session(db, session_config.task_id),
        None => Ok(()),
    }
}

/// Closes the open session of the task `tmux_session` belongs to, carrying a detach message (see
/// `tmux::DETACH_MESSAGE_OPTION`) onto it. Unknown names are ignored.
pub(crate) fn stop_for_tmux_session(db: &Db, tmux_session: &str) -> Result<()> {
    let Some(session_config) = db.find_session_config_by_tmux_name(tmux_session)? else {
        return Ok(()); // not one of ours -- ignore
    };
    let message = tmux::take_detach_message(tmux_session);
    close_open_session(db, session_config.task_id, message.as_deref())?;
    Ok(())
}

/// A client left `tmux_session`: stops its clock if that was the last client. The leaving client is
/// already off tmux's list when the hook runs, so what remains is exactly who is still working.
pub(crate) fn client_left_tmux_session(db: &Db, tmux_session: &str) -> Result<()> {
    if tmux::any_client_attached(tmux_session) {
        return Ok(()); // somebody else is still in there
    }
    stop_for_tmux_session(db, tmux_session)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::models::{Project, Task, TaskStatus};
    use chrono::NaiveDate;

    fn at(h: u32, m: u32, s: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 9, 1)
            .expect("valid date")
            .and_hms_opt(h, m, s)
            .expect("valid time")
    }

    /// An in-memory database with one task whose session opened at 09:00:00; returns it and the id.
    fn open_at_nine() -> (Db, i64) {
        let db = fixtures::db();
        let project_id = db
            .insert(&Project {
                tmux: false,
                auto_branch: false,
                ..fixtures::project("proj")
            })
            .expect("project inserts");
        let task = Task {
            status: TaskStatus::Wip,
            ..fixtures::task(project_id, "a task")
        };
        let task_id = db.insert(&task).expect("task inserts");
        fixtures::add_session(&db, task_id, at(9, 0, 0), None);
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

    #[test]
    fn a_stale_session_is_closed_at_the_last_save() {
        let (db, task_id) = open_at_nine();
        let closed = close_stale_session(&db, task_id, at(18, 0, 0), Some(at(11, 30, 0)))
            .expect("closing succeeds");
        assert_eq!(closed, Closed::Kept);
        let sessions = db.sessions_for_task(task_id).expect("lookup");
        assert_eq!(sessions[0].end, Some(at(11, 30, 0)));
    }

    /// No save, or one from before the session started: there's no known
    /// end, so the session goes rather than running to the restart.
    #[test]
    fn a_stale_session_without_a_usable_save_is_deleted() {
        for save in [None, Some(at(8, 0, 0))] {
            let (db, task_id) = open_at_nine();
            let closed =
                close_stale_session(&db, task_id, at(18, 0, 0), save).expect("closing succeeds");
            assert_eq!(closed, Closed::Discarded, "save {save:?}");
            assert!(db.sessions_for_task(task_id).expect("lookup").is_empty());
        }
    }

    #[test]
    fn a_session_the_running_server_opened_is_left_alone() {
        let (db, task_id) = open_at_nine();
        let closed = close_stale_session(&db, task_id, at(8, 0, 0), Some(at(8, 30, 0)))
            .expect("closing succeeds");
        assert_eq!(closed, Closed::Nothing);
        assert!(db.sessions_for_task(task_id).expect("lookup")[0].is_ongoing());
    }

    #[test]
    fn a_stale_session_saved_within_a_minute_of_starting_is_deleted() {
        let (db, task_id) = open_at_nine();
        let closed = close_stale_session(&db, task_id, at(18, 0, 0), Some(at(9, 0, 30)))
            .expect("closing succeeds");
        assert_eq!(closed, Closed::Discarded);
    }
}
