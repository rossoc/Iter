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
use chrono::Local;

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

pub(crate) fn close_open_session(db: &Db, task_id: i64, message: Option<&str>) -> Result<()> {
    if let Some(mut open) = db.open_session_for_task(task_id)? {
        debug_assert!(
            open.is_ongoing(),
            "open_session_for_task returned a closed session"
        );
        open.end = Some(Local::now().naive_local());
        if let Some(m) = message {
            open.message = Some(m.to_string());
        }
        db.update(open.id(), &open)?;
    }
    Ok(())
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
    close_open_session(db, session_config.task_id, message.as_deref())
}
