//! `iter session` -- setting up a task's workspace and tracking the time
//! spent in it.

use crate::Run;
use crate::args::SessionCommand;
use crate::ingest;
use iter_core::agent::ingest_job::Trigger;
use iter_core::app::App;
use iter_core::db::Table;
use iter_core::error::{IterError, Result};
use iter_core::locate::{self, current_task, require_git_repo, task_or_current};
use iter_core::models::{Duration, SessionConfig, TaskStatus};
use iter_core::tmux::{Multiplexer, TmuxClient};
use iter_core::workspace;
use iter_core::workspace::clock::{Closed, close_open_session, start_session};

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
            Self::Start { task } => session_start(app, task.name()),
            Self::Stop { task, message } => session_stop(app, task.name(), message.as_deref()),
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
            && existing
                .worktree_path
                .as_deref()
                .is_some_and(worktree_exists)
        {
            adopted = Some(existing);
        } else {
            // a restart or kill leaves a row with nothing to tear down; only a row whose tmux session and worktree are both gone is dropped
            let tmux_gone = existing
                .tmux_session_name
                .as_deref()
                .is_some_and(|name| !tmux.session_exists(name));
            let worktree_gone = !existing
                .worktree_path
                .as_deref()
                .is_some_and(worktree_exists);
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
        println!("{}", started_line(&tmux_session_name, adopt));
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

/// What `session new` says once the tmux session is up; a session started in the workspace an
/// agent made says so.
fn started_line(name: &str, adopted: bool) -> String {
    match adopted {
        true => format!("session '{name}' started in the workspace made for the agent"),
        false => format!("session '{name}' started"),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use iter_core::fixtures;
    use iter_core::models::{Project, Task};
    use iter_core::process::fake::FakeRunner;

    #[test]
    fn a_session_in_an_agents_workspace_says_so() {
        assert_eq!(started_line("web/1 x", false), "session 'web/1 x' started");
        assert_eq!(
            started_line("web/1 x", true),
            "session 'web/1 x' started in the workspace made for the agent"
        );
    }

    /// R4b of task 302: the first window comes with the session; each other window of
    /// `session.windows` is opened with `new-window` in the session, named, in the workspace,
    /// running its command, before the client attaches. The config has no `paste`, so no
    /// worker is started (a test must not start the test binary as `iter internal paste`).
    #[test]
    fn session_new_opens_the_configured_windows() {
        let yaml = "session:\n  windows:\n    - name: shell\n    - name: claude\n      command: claude\n    - name: edit\n      command: nvim\n";
        if fixtures::in_scratch_config(
            module_path!(),
            "session_new_opens_the_configured_windows",
            Some(yaml),
        ) {
            return;
        }
        let app = App::open().expect("the scratch config and database open");
        let project = Project {
            tmux: true,
            github: false,
            ..fixtures::project("web")
        };
        let project_id = app.db.insert(&project).unwrap();
        let task_id = fixtures::add_task(&app.db, project_id, "login");
        let runner = FakeRunner::default();
        runner.fail("tmux has-session");

        session_new(
            &app,
            &TmuxClient::new(&runner, false),
            "web/login",
            None,
            false,
        )
        .unwrap();

        let calls = runner.calls();
        let session = format!("web/{task_id} login");
        let at = |prefix: &str| {
            calls
                .iter()
                .position(|c| c.starts_with(prefix))
                .unwrap_or_else(|| panic!("{prefix} in {calls:?}"))
        };
        let created = at("tmux new-session");
        let attached = at("tmux attach-session");
        let windows: Vec<(usize, &String)> = calls
            .iter()
            .enumerate()
            .filter(|(_, c)| c.starts_with("tmux new-window"))
            .collect();
        assert_eq!(windows.len(), 2, "claude and edit, not shell: {calls:?}");
        for ((i, call), (name, command)) in
            windows.iter().zip([("claude", "claude"), ("edit", "nvim")])
        {
            assert!(
                created < *i && *i < attached,
                "between session and attach: {calls:?}"
            );
            for piece in [
                format!("-t ={session}"),
                format!("-n {name}"),
                "-c /tmp/web".to_string(),
            ] {
                assert!(call.contains(&piece), "{piece} in {call}");
            }
            assert!(call.ends_with(&format!(" {command}")), "{call}");
        }
        assert!(
            windows[0].0 < windows[1].0,
            "in the configured order: {calls:?}"
        );
        assert!(!calls.iter().any(|c| c.contains("-n shell")), "{calls:?}");
        assert!(
            !calls.iter().any(|c| c.contains("paste-buffer")),
            "{calls:?}"
        );
        let stored = app
            .db
            .find_session_config_by_task(task_id)
            .unwrap()
            .expect("the row is written");
        assert_eq!(stored.tmux_session_name.as_deref(), Some(session.as_str()));
    }

    /// R4c: the paste worker waits until the window runs its command (`claude`, from the
    /// default `session.windows`), then puts the prompt of `iter task prompt -i complete` in a
    /// buffer and pastes it bracketed into that window, and never presses Enter.
    #[test]
    fn the_paste_worker_pastes_the_prompt_once_claude_runs() {
        let notes = fixtures::scratch("session-paste");
        fixtures::install_instructions(&notes);
        let app = App::with_db(fixtures::db());
        let project = Project {
            tmux: true,
            notes_path: notes.display().to_string(),
            ..fixtures::project("web")
        };
        let project_id = app.db.insert(&project).unwrap();
        let task_id = app
            .db
            .insert(&Task {
                description: "Paste me, do not send me.".into(),
                ..fixtures::task(project_id, "login")
            })
            .unwrap();
        let session = format!("web/{task_id} login");
        app.db
            .insert(&SessionConfig {
                id: None,
                task_id,
                tmux_session_name: Some(session.clone()),
                github_branch: None,
                worktree_path: None,
            })
            .unwrap();
        let wanted = iter_core::agent::task_prompt::prompt_text(
            &app,
            Some("web/login"),
            "complete",
            None,
            true,
        )
        .unwrap();
        assert!(wanted.contains("Paste me, do not send me."));
        let runner = FakeRunner::default();
        runner.reply("tmux display", "claude\n");
        runner.reply("tmux list-panes", "claude\n");

        paste(
            &app,
            &TmuxClient::new(&runner, false),
            &session,
            "claude",
            "complete",
        )
        .unwrap();

        let calls = runner.calls();
        let at = |prefix: &str| calls.iter().position(|c| c.starts_with(prefix));
        let set = at("tmux set-buffer").unwrap_or_else(|| panic!("set-buffer: {calls:?}"));
        let pasted = at("tmux paste-buffer").unwrap_or_else(|| panic!("paste-buffer: {calls:?}"));
        let asked = at("tmux display")
            .or_else(|| at("tmux list-panes"))
            .unwrap_or_else(|| panic!("asked what the pane runs: {calls:?}"));
        assert!(asked < set && set < pasted, "{calls:?}");
        assert!(
            calls[set].ends_with(&wanted),
            "the whole prompt: {}",
            calls[set]
        );
        assert!(
            calls[pasted].contains(" -p"),
            "bracketed: {}",
            calls[pasted]
        );
        assert!(
            calls[pasted].contains(&format!("{session}:claude")),
            "into the claude window: {}",
            calls[pasted]
        );
        assert!(!calls.iter().any(|c| c.contains("send-keys")), "{calls:?}");
        let _ = std::fs::remove_dir_all(&notes);
    }

    /// `task solve` left a worktree and a row with no tmux name: `session new` keeps both, starts
    /// the tmux session in the worktree, names it in the row and moves the task out of the queue.
    #[test]
    fn session_new_adopts_the_workspace_an_agent_made() {
        let worktree = fixtures::scratch("session-adopt");
        std::fs::create_dir_all(&worktree).unwrap();
        let worktree_path = worktree.display().to_string();
        let app = App::with_db(fixtures::db());
        let project = Project {
            github: true,
            tmux: true,
            ..fixtures::project("web")
        };
        let project_id = app.db.insert(&project).unwrap();
        let task_id = fixtures::add_task(&app.db, project_id, "login");
        let config = SessionConfig {
            id: None,
            task_id,
            tmux_session_name: None,
            github_branch: Some("feat/1-login".into()),
            worktree_path: Some(worktree_path.clone()),
        };
        app.db.insert(&config).unwrap();
        let runner = FakeRunner::default();
        runner.fail("tmux has-session");

        session_new(
            &app,
            &TmuxClient::new(&runner, false),
            "web/login",
            None,
            false,
        )
        .unwrap();

        let calls = runner.calls();
        let new_session = format!("tmux new-session -d -s web/{task_id} login -c {worktree_path}");
        assert!(calls.contains(&new_session), "{calls:?}");
        assert!(
            calls.iter().any(|c| c.starts_with("tmux attach-session")),
            "{calls:?}"
        );
        let stored = app
            .db
            .find_session_config_by_task(task_id)
            .unwrap()
            .unwrap();
        assert_eq!(
            stored.tmux_session_name.as_deref(),
            Some(format!("web/{task_id} login").as_str())
        );
        assert_eq!(
            stored.worktree_path.as_deref(),
            Some(worktree_path.as_str())
        );
        let task: Task = app.db.get(task_id).unwrap().unwrap();
        assert_eq!(task.status, TaskStatus::Wip);
        let _ = std::fs::remove_dir_all(&worktree);
    }
}
