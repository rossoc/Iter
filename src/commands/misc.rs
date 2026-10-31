//! The commands that belong to no entity: `iter comment`, `iter t`, and
//! the hidden `iter internal hook`/`kill-server`/`restore` that tmux invokes.

use crate::app::App;
use crate::args::{HookEvent, InternalCommand};
use crate::commands::Run;
use crate::error::Result;
use crate::github::{GhCli, IssueTracker};
use crate::models::{Project, SessionConfig, Task};
use crate::tmux::{Multiplexer, TmuxClient};
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
    GhCli::system().comment(&project.base_path, issue, message)?;
    println!("posted comment on issue #{issue}");
    Ok(())
}

pub(crate) fn t_cmd(app: &App) -> Result<()> {
    let db = &app.db;
    let (project, task) = current_session_task(db)?;
    println!("{}", task_display(&project, &task));
    Ok(())
}

fn hook_cmd(app: &App, tmux: &dyn Multiplexer, event: &HookEvent) -> Result<()> {
    let db = &app.db;
    match event {
        // fires on attach and `switch-client` alike. The session left is closed before the one entered opens;
        // `previous` is empty on a first attach, equal to `session` when a switch stays put
        HookEvent::ClientSessionChanged {
            client,
            session,
            previous,
        } => {
            tmux.remember_client_session(client, session)?;
            if let Some(previous) = previous
                .as_deref()
                .filter(|p| !p.is_empty() && p != session)
            {
                client_left_tmux_session(db, previous)?;
            }
            start_for_tmux_session(db, session)
        }
        HookEvent::ClientDetached { client } => match tmux.take_client_session(client) {
            Some(session) => client_left_tmux_session(db, &session),
            None => Ok(()), // attached before the hooks were installed
        },
        HookEvent::SessionClosed { session } => stop_for_tmux_session(db, session),
    }
}

/// Stops every tmux session's clock, then kills the server: tmux fires no hook when the server goes away with clients attached.
fn kill_server_cmd(app: &App, tmux: &dyn Multiplexer) -> Result<()> {
    // one session failing to stop must not leave the server behind: kill it regardless, report the first error
    let mut stopped: Result<()> = Ok(());
    for session in tmux.list_sessions() {
        let result = stop_for_tmux_session(&app.db, &session);
        if stopped.is_ok() {
            stopped = result;
        }
    }
    tmux.kill_server()?;
    stopped
}

/// Puts iter back on a tmux server that just came up (e.g. tmux-resurrect restored the sessions by name, which reconnects tasks);
/// redoes everything else. Safe to repeat.
fn restore_cmd(app: &App, tmux: &dyn Multiplexer) -> Result<()> {
    let db = &app.db;
    let tracked: Vec<(SessionConfig, String)> = db
        .list::<SessionConfig>()?
        .into_iter()
        .filter_map(|config| {
            let name = config.tmux_session_name.clone()?;
            Some((config, name))
        })
        .collect();

    // a session newer than resurrect's last save is started again
    for (config, name) in &tracked {
        if tmux.session_exists(name) {
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
            Some(cwd) => tmux.create_session(name, &cwd)?,
            None => eprintln!("note: not recreating '{name}': its directory is gone"),
        }
    }

    // hooks live in the server, so the old server's went with it
    tmux.install_hooks_for_current_exe()?;

    if let Some(server_start) = tmux.server_start_time() {
        let last_save = tmux.last_resurrect_save();
        for (config, _) in &tracked {
            close_stale_session(db, config.task_id, server_start, last_save)?;
        }
    }

    // resurrect re-attaches the client before the hooks existed, so that attach was never seen
    for (_, name) in &tracked {
        if tmux.any_client_attached(name) {
            start_for_tmux_session(db, name)?;
        }
    }
    Ok(())
}

impl Run for InternalCommand {
    fn run(&self, app: &App) -> Result<()> {
        match self {
            Self::Hook { event } => hook_cmd(app, &TmuxClient::system(), event),
            Self::KillServer => kill_server_cmd(app, &TmuxClient::system()),
            Self::Restore => restore_cmd(app, &TmuxClient::system()),
        }
    }
}
