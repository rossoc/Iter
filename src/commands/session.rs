//! `iter session` -- setting up a task's workspace and tracking the time
//! spent in it.

use crate::app::App;
use crate::args::SessionCommand;
use crate::commands::Run;
use crate::commands::ingest;
use crate::db::Table;
use crate::error::{IterError, Result};
use crate::locate::{self, current_task, require_git_repo, task_or_current};
use crate::models::{Duration, SessionConfig, TaskStatus};
use crate::tmux::{Multiplexer, TmuxClient};
use crate::wiki::ingest_job::Trigger;
use crate::workspace;
use crate::workspace::clock::{Closed, close_open_session, start_session};

impl Run for SessionCommand {
    fn run(&self, app: &App) -> Result<()> {
        match self {
            Self::New {
                task,
                branch,
                no_branch,
            } => session_new(
                app,
                &TmuxClient::system(),
                task,
                branch.as_deref(),
                *no_branch,
            ),
            Self::Start { task } => session_start(app, task.as_deref()),
            Self::Stop { task, message } => session_stop(app, task.as_deref(), message.as_deref()),
            Self::Elapse => session_elapse(app),
        }
    }
}

fn session_new(
    app: &App,
    tmux: &dyn Multiplexer,
    task_ref: &str,
    branch_override: Option<&str>,
    no_branch: bool,
) -> Result<()> {
    let db = &app.db;
    let (project, mut task) = locate::task(db, &task_ref.parse()?)?;
    let task_id = task.id();

    // an agent's workspace (`iter task solve`) has a worktree and no tmux session: give it one
    let mut adopted = None;
    if let Some(existing) = db.find_session_config_by_task(task_id)? {
        let worktree_exists = |path: &str| std::path::Path::new(path).exists();
        if project.tmux
            && existing.tmux_session_name.is_none()
            && existing.worktree_path.as_deref().is_some_and(worktree_exists)
        {
            adopted = Some(existing);
        } else {
            // a restart or kill leaves a row with nothing to tear down; only a row whose tmux session and worktree are both gone is dropped
            let tmux_gone = existing
                .tmux_session_name
                .as_deref()
                .is_some_and(|name| !tmux.session_exists(name));
            let worktree_gone = !existing.worktree_path.as_deref().is_some_and(worktree_exists);
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
    }
    let adopt = adopted.is_some();

    let tmux_session_name = workspace::tmux_name(&project, &task);

    if project.github && !adopt {
        require_git_repo(&project)?;
    }

    if project.tmux && tmux.session_exists(&tmux_session_name) {
        return Err(IterError::TmuxSessionExists(tmux_session_name));
    }

    let mut config = match adopted {
        Some(config) => config,
        None if project.github && project.auto_branch && !no_branch => {
            workspace::open(db, &project, &task, branch_override)?
        }
        None => SessionConfig {
            id: None,
            task_id,
            tmux_session_name: None,
            github_branch: None,
            worktree_path: None,
        },
    };
    let cwd = config
        .worktree_path
        .clone()
        .unwrap_or_else(|| project.base_path.clone());

    // a fresh worktree and branch are the row's: a failure from here on takes them down again
    // (retry: "branch already exists"); an agent's workspace is only left without its session
    let undo = |config: &SessionConfig, session: Option<&str>| match adopt {
        true => session.into_iter().for_each(|name| tmux.kill_session(name)),
        false => workspace::unwind(db, tmux, &project, config, session),
    };
    if project.tmux {
        tmux.start_tracked_session(&tmux_session_name, &cwd)
            .inspect_err(|_| undo(&config, None))?;
        println!("session '{tmux_session_name}' started");
        config.tmux_session_name = Some(tmux_session_name);
    } else {
        println!(
            "session started for '{task_ref}' at {cwd} (tmux disabled for this project -- use `iter session start`/`iter session stop`)"
        );
    }

    match config.id {
        Some(id) => db.update(id, &config),
        None => db.insert(&config).map(drop),
    }
    .inspect_err(|_| undo(&config, config.tmux_session_name.as_deref()))?;

    match adopt {
        true => workspace::start_work(db, &mut task)?,
        false => {
            task.status = TaskStatus::Wip;
            db.update(task_id, &task)?;
            ingest::auto(app, &project, &task, Trigger::TaskStart);
        }
    }

    // attach last, once every row is written: the `client-attached` hook looks the session-config up by tmux name
    if let Some(name) = &config.tmux_session_name
        && let Err(e) = tmux.attach_session(name)
    {
        eprintln!("warning: couldn't attach to '{name}': {e}");
        eprintln!("the session is set up -- attach with: tmux attach -t '{name}'");
    }

    Ok(())
}

fn session_start(app: &App, task_ref: Option<&str>) -> Result<()> {
    let db = &app.db;
    let (project, task) = task_or_current(db, task_ref)?;
    start_session(db, task.id())?;
    println!("started a session for '{}'", task.label(&project));
    Ok(())
}

fn session_stop(app: &App, task_ref: Option<&str>, message: Option<&str>) -> Result<()> {
    let db = &app.db;
    let (project, task) = task_or_current(db, task_ref)?;
    let what = match close_open_session(db, task.id(), message)? {
        Closed::Discarded => "discarded the session (under a minute) for",
        Closed::Kept | Closed::Nothing => "stopped the session for",
    };
    println!("{what} '{}'", task.label(&project));
    Ok(())
}

/// Time elapsed on the open session of the task of the tmux session this process runs in.
fn session_elapse(app: &App) -> Result<()> {
    let db = &app.db;
    let (project, task) = current_task(db)?;
    let task_id = task.id();
    let display = task.label(&project).to_string();
    let session = db
        .open_session_for_task(task_id)?
        .ok_or_else(|| IterError::NoOpenSession(display))?;
    let now = app.now;
    println!("{}", Duration(session.duration_minutes(now)));
    Ok(())
}
