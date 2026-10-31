//! The commands that belong to no entity: `iter comment`, `iter t`, and
//! the hidden `iter internal hook`/`kill-server`/`restore` that tmux invokes.

use crate::app::App;
use crate::args::{HookEvent, InternalCommand};
use crate::commands::Run;
use crate::error::Result;
use crate::github;
use crate::models::{Project, SessionConfig, Task};
use crate::tmux;
use crate::utils::clock::{
    client_left_tmux_session, close_stale_session, start_for_tmux_session, stop_for_tmux_session,
};
use crate::utils::resolve::{
    current_session_task, linked_issue, require_github, resolve_task_or_current, task_display,
};
use std::path::Path;

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
    // One session failing to stop must not leave the server (and the other
    // sessions' clocks) behind: kill it regardless, then report the first error.
    let stopped: Result<()> = tmux::list_sessions()
        .iter()
        .map(|session| stop_for_tmux_session(&app.db, session))
        .fold(Ok(()), |first, next| first.and(next));
    tmux::kill_server()?;
    stopped
}

/// Puts iter back on a tmux server that has just come up -- after a reboot,
/// typically, with tmux-resurrect restoring the sessions by name. The names
/// are what tie a task to its tmux session, so they reconnect by
/// themselves; what doesn't survive is everything else, and that's what
/// this redoes. Safe to run again: each step leaves alone what's already
/// right.
fn restore_cmd(app: &App) -> Result<()> {
    let db = &app.db;
    let tracked: Vec<(SessionConfig, String)> = db
        .list::<SessionConfig>()?
        .into_iter()
        .filter_map(|config| {
            let name = config.tmux_session_name.clone()?;
            Some((config, name))
        })
        .collect();

    // A session newer than resurrect's last save isn't in it: start it
    // again where `session new` did.
    for (config, name) in &tracked {
        if tmux::session_exists(name) {
            continue;
        }
        let base_path = match db.get::<Task>(config.task_id)? {
            Some(task) => db.get::<Project>(task.project_id)?.map(|p| p.base_path),
            None => None,
        };
        let cwd = [config.worktree_path.clone(), base_path]
            .into_iter()
            .flatten()
            .find(|path| Path::new(path).is_dir());
        match cwd {
            Some(cwd) => tmux::create_session(name, &cwd)?,
            None => eprintln!("note: not recreating '{name}': its directory is gone"),
        }
    }

    // `set-hook -g` lives in the server, so the old server's hooks went
    // with it.
    tmux::install_hooks_for_current_exe()?;

    if let Some(server_start) = tmux::server_start_time() {
        let last_save = tmux::last_resurrect_save();
        for (config, _) in &tracked {
            close_stale_session(db, config.task_id, server_start, last_save)?;
        }
    }

    // resurrect puts the client back on its session before the hooks above
    // existed, so that attach was never seen.
    for (_, name) in &tracked {
        if tmux::any_client_attached(name) {
            start_for_tmux_session(db, name)?;
        }
    }
    Ok(())
}

impl Run for InternalCommand {
    fn run(&self, app: &App) -> Result<()> {
        match self {
            Self::Hook { event } => hook_cmd(app, event),
            Self::KillServer => kill_server_cmd(app),
            Self::Restore => restore_cmd(app),
        }
    }
}
