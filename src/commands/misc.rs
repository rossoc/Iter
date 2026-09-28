//! The commands that belong to no entity: `iter comment`, `iter t`, and
//! the hidden `iter internal hook`/`kill-server` that tmux invokes.

use crate::app::App;
use crate::args::{HookEvent, InternalCommand};
use crate::commands::Run;
use crate::error::Result;
use crate::github;
use crate::tmux;
use crate::utils::clock::{
    client_left_tmux_session, start_for_tmux_session, stop_for_tmux_session,
};
use crate::utils::resolve::{
    current_session_task, linked_issue, require_github, resolve_task_or_current, task_display,
};

pub(crate) fn comment_cmd(app: &App, task_ref: Option<&str>, message: &str) -> Result<()> {
    let db = &app.db;
    let (project, task) = resolve_task_or_current(db, task_ref)?;
    require_github(&project)?;
    let issue = linked_issue(&project, &task)?;
    github::post_comment(&project.base_path, issue, message)?;
    println!("posted comment on issue #{issue}");
    Ok(())
}

pub(crate) fn t_cmd(app: &App) -> Result<()> {
    let db = &app.db;
    let (project, task) = current_session_task(db)?;
    println!("{}", task_display(&project, &task));
    Ok(())
}

fn hook_cmd(app: &App, event: &HookEvent) -> Result<()> {
    let db = &app.db;
    match event {
        // Fires on a plain attach and on `switch-client` alike. The session
        // being left is closed before the one being entered opens, so
        // hopping between two tasks doesn't leave both of them running --
        // `previous` is empty on a first attach, and equal to `session` on a
        // switch that stays put.
        HookEvent::ClientSessionChanged {
            client,
            session,
            previous,
        } => {
            tmux::remember_client_session(client, session)?;
            if let Some(previous) = previous
                .as_deref()
                .filter(|p| !p.is_empty() && p != session)
            {
                client_left_tmux_session(db, previous)?;
            }
            start_for_tmux_session(db, session)
        }
        HookEvent::ClientDetached { client } => match tmux::take_client_session(client) {
            Some(session) => client_left_tmux_session(db, &session),
            None => Ok(()), // attached before the hooks were installed
        },
        HookEvent::SessionClosed { session } => stop_for_tmux_session(db, session),
    }
}

/// Stops every tmux session's clock, then kills the server. Clients are
/// still attached at this point, so this stops them outright rather than
/// waiting for the last one out -- the server going away is everyone
/// leaving at once, and tmux fires no hook that would say so.
fn kill_server_cmd(app: &App) -> Result<()> {
    for session in tmux::list_sessions() {
        stop_for_tmux_session(&app.db, &session)?;
    }
    tmux::kill_server()
}

impl Run for InternalCommand {
    fn run(&self, app: &App) -> Result<()> {
        match self {
            Self::Hook { event } => hook_cmd(app, event),
            Self::KillServer => kill_server_cmd(app),
        }
    }
}
