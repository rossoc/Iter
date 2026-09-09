//! The commands that belong to no entity: `iter comment`, `iter t`, and
//! the hidden `iter internal hook` the tmux hooks invoke.

use crate::app::App;
use crate::args::InternalCommand;
use crate::commands::Run;
use crate::error::{IterError, Result};
use crate::github;
use crate::utils::clock::{start_for_tmux_session, stop_for_tmux_session};
use crate::utils::resolve::{
    current_session_task, require_github, resolve_task_or_current, task_display,
};

pub(crate) fn comment_cmd(app: &App, task_ref: Option<&str>, message: &str) -> Result<()> {
    let db = &app.db;
    let (project, task) = resolve_task_or_current(db, task_ref)?;
    require_github(&project)?;
    let issue = task
        .github_issue
        .ok_or_else(|| IterError::NoLinkedIssue(task_display(&project, &task)))?;
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

fn hook_cmd(app: &App, event: &str, tmux_session: &str, previous: Option<&str>) -> Result<()> {
    let db = &app.db;
    match event {
        // Fires on a plain attach and on `switch-client` alike. The session
        // being left is closed before the one being entered opens, so
        // hopping between two tasks doesn't leave both of them running --
        // `previous` is empty on a first attach, and equal to
        // `tmux_session` on a switch that stays put.
        "client-session-changed" => {
            if let Some(previous) = previous.filter(|p| !p.is_empty() && *p != tmux_session) {
                stop_for_tmux_session(db, previous)?;
            }
            start_for_tmux_session(db, tmux_session)
        }
        // Kept for a server still running a hook an older `iter` installed;
        // `ensure_hooks_installed` no longer sets this one.
        "client-attached" => start_for_tmux_session(db, tmux_session),
        "client-detached" | "session-closed" => stop_for_tmux_session(db, tmux_session),
        _ => Ok(()),
    }
}

impl Run for InternalCommand {
    fn run(&self, app: &App) -> Result<()> {
        match self {
            Self::Hook {
                event,
                tmux_session,
                previous,
            } => hook_cmd(app, event, tmux_session, previous.as_deref()),
        }
    }
}
