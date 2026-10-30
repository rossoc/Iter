//! `iter session` -- setting up a task's workspace and tracking the time
//! spent in it.

use crate::app::App;
use crate::args::SessionCommand;
use crate::commands::Run;
use crate::db::Table;
use crate::error::{IterError, Result};
use crate::models::{Duration, Project, SessionConfig, Task, TaskStatus};
use crate::tmux;
use crate::utils::clock::{Closed, close_open_session, start_session};
use crate::utils::resolve::{
    current_session_task, require_git_repo, resolve_task, resolve_task_or_current, task_display,
};
use crate::utils::workspace::{self, unwind_session_setup};

impl Run for SessionCommand {
    fn run(&self, app: &App) -> Result<()> {
        match self {
            Self::New {
                task,
                branch,
                no_branch,
            } => session_new(app, task, branch.as_deref(), *no_branch),
            Self::Start { task } => session_start(app, task.as_deref()),
            Self::Stop { task, message } => session_stop(app, task.as_deref(), message.as_deref()),
            Self::Elapse => session_elapse(app),
        }
    }
}

fn session_new(
    app: &App,
    task_ref: &str,
    branch_override: Option<&str>,
    no_branch: bool,
) -> Result<()> {
    let db = &app.db;
    let (project, mut task) = resolve_task(db, task_ref)?;
    let task_id = task.id();

    if let Some(existing) = db.find_session_config_by_task(task_id)? {
        // A tmux restart or kill leaves the row behind with nothing left to
        // tear down; only a row whose tmux session and worktree are both gone
        // is dropped, so a live workspace still blocks a second `session new`.
        let tmux_gone = existing
            .tmux_session_name
            .as_deref()
            .is_some_and(|name| !tmux::session_exists(name));
        let worktree_gone = existing
            .worktree_path
            .as_deref()
            .is_none_or(|path| !std::path::Path::new(path).exists());
        match (tmux_gone, worktree_gone, existing.id) {
            (true, true, Some(id)) => {
                db.delete::<SessionConfig>(id)?;
                eprintln!(
                    "note: dropped a stale session for '{task_ref}' (its tmux session and worktree are gone)"
                );
            }
            _ => return Err(IterError::SessionAlreadyExists(task_ref.to_string())),
        }
    }

    let tmux_session_name = tmux_session_name(&project, &task);

    if project.github {
        require_git_repo(&project)?;
    }

    if project.tmux && tmux::session_exists(&tmux_session_name) {
        return Err(IterError::TmuxSessionExists(tmux_session_name));
    }

    let mut github_branch = None;
    let mut worktree_path = None;

    if project.github && project.auto_branch && !no_branch {
        let (branch, path) = workspace::create_task_worktree(&project, &task, branch_override)?;
        github_branch = Some(branch);
        worktree_path = Some(path);
    }

    // Borrowed, not cloned: `cwd` only needs to outlive this function, and
    // both `worktree_path` and `project.base_path` already do.
    let cwd: &str = worktree_path.as_deref().unwrap_or(&project.base_path);

    // Everything from here to the insert can fail, and until that row
    // exists nothing else knows the worktree and branch are there --
    // `teardown_session_config` works off the row, so a failure that left
    // them behind would leave them behind for good, and the retry would hit
    // "branch already exists" on a branch the user has to clean up by hand.
    let final_tmux_name = if project.tmux {
        tmux::start_tracked_session(&tmux_session_name, cwd).inspect_err(|_| {
            unwind_session_setup(
                &project,
                worktree_path.as_deref(),
                github_branch.as_deref(),
                Some(&tmux_session_name),
            )
        })?;
        println!("session '{tmux_session_name}' started");
        Some(tmux_session_name)
    } else {
        println!(
            "session started for '{task_ref}' at {cwd} (tmux disabled for this project -- use `iter session start`/`iter session stop`)"
        );
        None
    };

    let session_config = SessionConfig {
        id: None,
        task_id,
        tmux_session_name: final_tmux_name,
        github_branch,
        worktree_path,
    };
    db.insert(&session_config).inspect_err(|_| {
        unwind_session_setup(
            &project,
            session_config.worktree_path.as_deref(),
            session_config.github_branch.as_deref(),
            session_config.tmux_session_name.as_deref(),
        )
    })?;

    task.status = TaskStatus::Wip;
    db.update(task_id, &task)?;

    // Attaching last, and only once every row is written: it hands the
    // terminal over to tmux until the user detaches, and the
    // `client-attached` hook that fires on the way in looks the
    // session-config up by tmux name -- so the row has to be there first.
    if let Some(name) = &session_config.tmux_session_name
        && let Err(e) = tmux::attach_session(name)
    {
        eprintln!("warning: couldn't attach to '{name}': {e}");
        eprintln!("the session is set up -- attach with: tmux attach -t '{name}'");
    }

    Ok(())
}

/// The tmux session name for a task. Deliberately *not* `task_display`:
/// tmux treats `/` as a hierarchy separator, so each half has its own
/// slashes flattened and the one that remains is the separator. The result
/// is a tmux identifier, not a `<project>/<task>` ref, and
/// `parse_task_ref` is not its inverse.
fn tmux_session_name(project: &Project, task: &Task) -> String {
    format!(
        "{}/{}",
        tmux_name_part(&project.name),
        tmux_name_part(&task.name)
    )
}

/// One half of a [`tmux_session_name`]. Besides `/`, it flattens `:` and
/// `.`: `new-session -s` takes any name, but every `-t` after it is parsed as
/// `session:window.pane`, so a task called `web: x` makes a session nothing
/// can attach to, check or kill ("can't find window:  x").
fn tmux_name_part(name: &str) -> String {
    name.replace('/', "-").replace([':', '.'], "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_part_has_no_tmux_target_separators() {
        assert_eq!(tmux_name_part("web: a.b/c"), "web_ a_b-c");
    }
}

fn session_start(app: &App, task_ref: Option<&str>) -> Result<()> {
    let db = &app.db;
    let (project, task) = resolve_task_or_current(db, task_ref)?;
    start_session(db, task.id())?;
    println!("started a session for '{}'", task_display(&project, &task));
    Ok(())
}

fn session_stop(app: &App, task_ref: Option<&str>, message: Option<&str>) -> Result<()> {
    let db = &app.db;
    let (project, task) = resolve_task_or_current(db, task_ref)?;
    let what = match close_open_session(db, task.id(), message)? {
        Closed::Discarded => "discarded the session (under a minute) for",
        Closed::Kept | Closed::Nothing => "stopped the session for",
    };
    println!("{what} '{}'", task_display(&project, &task));
    Ok(())
}

/// Time elapsed on the open session of the task of the tmux session this
/// process is running in -- the session `start_session` created when the
/// client attached (see `hook_cmd`).
fn session_elapse(app: &App) -> Result<()> {
    let db = &app.db;
    let (project, task) = current_session_task(db)?;
    let task_id = task.id();
    let display = task_display(&project, &task);
    let session = db
        .open_session_for_task(task_id)?
        .ok_or_else(|| IterError::NoOpenSession(display))?;
    let now = app.now;
    println!("{}", Duration(session.duration_minutes(now)));
    Ok(())
}
