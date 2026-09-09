//! `iter session` -- setting up a task's workspace and tracking the time
//! spent in it.

use crate::app::App;
use crate::args::SessionCommand;
use crate::commands::Run;
use crate::db::Table;
use crate::error::{IterError, Result};
use crate::models::{Project, SessionConfig, Task, TaskStatus};
use crate::reporting::minutes_to_hhmm;
use crate::utils::clock::{close_open_session, start_session};
use crate::utils::resolve::{
    current_session_task, require_git_repo, resolve_task, resolve_task_or_current, task_display,
};
use crate::utils::workspace::unwind_session_setup;
use crate::{git, tmux};

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

    if db.find_session_config_by_task(task_id)?.is_some() {
        return Err(IterError::SessionAlreadyExists(task_ref.to_string()));
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
        // The task's own prefix is what it was created (and possibly
        // edited) with; an empty one -- a task from before the field
        // existed, or one the user blanked -- falls back to whatever the
        // project's template says now.
        let prefix = match task.branch_prefix.trim() {
            "" => git::branch_prefix(&project.branch_template),
            prefix => prefix.to_string(),
        };
        // One slug for the branch and the directory both. Two tasks whose
        // names slugify the same ("Fix login" and "fix-login") would
        // otherwise collide on both at once, and the second `session new`
        // would fail on a `git worktree add` the user can't work around --
        // `-b` renames the branch, but nothing renames the directory.
        let slug = git::task_slug(&task.name, task_id);
        let worktrees = std::path::Path::new(&project.base_path).join(".iter-worktrees");
        let slug = match worktrees.join(&slug).exists() {
            true => format!("{slug}-{task_id}"),
            false => slug,
        };
        let branch = branch_override
            .map(|b| b.to_string())
            .unwrap_or_else(|| format!("{prefix}{slug}"));
        let path_str = worktrees.join(&slug).to_string_lossy().to_string();
        git::create_worktree(&project.base_path, &branch, &path_str)?;
        github_branch = Some(branch);
        worktree_path = Some(path_str);
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
        project.name.replace('/', "-"),
        task.name.replace('/', "-")
    )
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
    close_open_session(db, task.id(), message)?;
    println!(
        "stopped the session for '{}'",
        task_display(&project, &task)
    );
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
    println!("{}", minutes_to_hhmm(session.duration_minutes(now)));
    Ok(())
}
