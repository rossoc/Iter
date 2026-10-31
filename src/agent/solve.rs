//! The life of an agent run: `create`, `amend`, `begin`, `cancel` and the detached `worker`; shared by `iter task solve` and the web page.
//! Callers hear what happens through an [`Observer`]; nothing prints here.

use crate::agent::error::AgentError;
use crate::agent::exec;
use crate::agent::prompt::Kind;
use crate::agent::run::{Harness, Invocation, RunRecord, Status};
use crate::agent::task_prompt::{locate, prompt_for};
use crate::agent::taskdir::TaskDir;
use crate::app::App;
use crate::db::Table;
use crate::error::{IterError, Result};
use crate::git;
use crate::models::{Project, SessionConfig, Task, TaskStatus};
use crate::utils::workspace::create_task_worktree;
use crate::wiki::ingest_job::{Auto, Trigger, auto};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// What a caller may hear while a run begins; every method defaults to nothing.
pub trait Observer {
    fn ingest(&self, _project: &Project, _outcome: &Auto) {}
    fn working(&self, _name: &str, _run: &RunRecord) {}
}

/// How a run begins: waited for or detached, harness settings, and who is told.
pub struct Launch<'a> {
    pub wait: bool,
    pub extra: &'a [String],
    pub say: &'a dyn Observer,
}

impl Launch<'_> {
    #[cfg(feature = "web")]
    pub const fn detached() -> Launch<'static> {
        Launch {
            wait: false,
            extra: &[],
            say: &Quiet,
        }
    }
}

#[cfg(feature = "web")]
pub struct Quiet;

#[cfg(feature = "web")]
impl Observer for Quiet {}

/// A run's harness, model and effort, and the front matter of the buffer `solve` opens.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Spec {
    pub agent: Harness,
    pub model: String,
    pub effort: String,
}

/// The run's worktree as `(branch, path)`: the task's own, else a new one recorded so `task done` finds it; neither without branches.
fn workspace(
    app: &App,
    project: &Project,
    task: &mut Task,
) -> Result<(Option<String>, Option<String>)> {
    let db = &app.db;
    if let Some(existing) = db.find_session_config_by_task(task.id())? {
        return Ok((existing.github_branch, existing.worktree_path));
    }
    if !(project.github && project.auto_branch) {
        return Ok((None, None));
    }
    if !git::is_git_repo(&project.base_path) {
        return Err(AgentError::NotAGitRepo(project.base_path.clone()).into());
    }
    let (branch, path) = create_task_worktree(project, task, None)?;
    let row = SessionConfig {
        id: None,
        task_id: task.id(),
        tmux_session_name: None,
        github_branch: Some(branch.clone()),
        worktree_path: Some(path.clone()),
    };
    if let Err(e) = db.insert(&row) {
        crate::utils::workspace::remove_worktree_and_branch(project, Some(&path), Some(&branch));
        return Err(e);
    }
    if task.status == TaskStatus::Queue {
        task.status = TaskStatus::Wip;
        db.update(task.id(), task)?;
    }
    Ok((Some(branch), Some(path)))
}

/// Marks a queued task in progress when an agent starts on it; a done task stays done.
fn start_work(app: &App, task: &mut Task) -> Result<()> {
    if task.status == TaskStatus::Queue {
        task.status = TaskStatus::Wip;
        app.db.update(task.id(), task)?;
    }
    Ok(())
}

fn now() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Starts a run of `kind` on `name` (`create` then `begin`), detached unless `wait`; returns its number. One run at a time per task.
pub fn start(
    app: &App,
    name: &str,
    kind: Kind,
    spec: &Spec,
    description: &str,
    launch: &Launch,
) -> Result<u32> {
    let (_, _, dir) = locate(app, Some(name))?;
    if let Some(r) = dir.runs().iter().find(|r| r.is_running()) {
        return Err(AgentError::RunBusy {
            run: r.n,
            task: name.to_string(),
        }
        .into());
    }
    let n = create(app, name, kind, spec, description, None, None)?;
    begin(app, name, n, launch)?;
    Ok(n)
}

/// Writes a run down without starting it (`queued`); `{{worktree}}` in the prompt is resolved at `begin`.
pub fn create(
    app: &App,
    name: &str,
    kind: Kind,
    spec: &Spec,
    description: &str,
    prompt: Option<&str>,
    instruction: Option<&str>,
) -> Result<u32> {
    let (_, _, dir) = locate(app, Some(name))?;
    dir.ensure()?;
    let text = match prompt {
        Some(text) if !text.trim().is_empty() => text.to_string(),
        _ => prompt_for(app, Some(name), kind, None, true, Some(description), true)?,
    };
    let n = dir.claim_run()?;
    let run = RunRecord {
        n,
        agent: spec.agent,
        model: spec.model.clone(),
        effort: spec.effort.clone(),
        kind,
        instruction: instruction.map(String::from),
        status: Status::Queued,
        started: now(),
        ended: None,
        exit_code: None,
        branch: None,
        worktree: None,
        pid: None,
    };
    let written = std::fs::write(dir.run_prompt_path(n), text)
        .map_err(IterError::from)
        .and_then(|()| dir.write_run(&run).map_err(IterError::from));
    if let Err(e) = written {
        // a number claimed and never filled in would sit in the list as a broken record
        let _ = std::fs::remove_file(dir.run_path(n));
        let _ = std::fs::remove_file(dir.run_prompt_path(n));
        return Err(e);
    }
    Ok(n)
}

/// Changes a queued run; one that has begun cannot be edited.
#[cfg(feature = "web")]
pub fn amend(
    app: &App,
    name: &str,
    n: u32,
    kind: Kind,
    instruction: &str,
    spec: &Spec,
    prompt: &str,
) -> Result<()> {
    let (_, _, dir) = locate(app, Some(name))?;
    let mut run = dir.read_run(n)?.ok_or(AgentError::RunNotFound(n))?;
    if !run.is_queued() {
        return Err(AgentError::RunBegun(n).into());
    }
    run.agent = spec.agent;
    run.model = spec.model.clone();
    run.effort = spec.effort.clone();
    run.kind = kind;
    run.instruction = Some(instruction.to_string());
    std::fs::write(dir.run_prompt_path(n), prompt)?;
    dir.write_run(&run)?;
    Ok(())
}

/// Begins queued run `n`: makes the workspace if the kind changes code, starts the harness (detached unless `wait`); a run that cannot begin is recorded as failed.
pub fn begin(app: &App, name: &str, n: u32, launch: &Launch) -> Result<()> {
    let Launch { wait, extra, say } = *launch;
    let (project, mut task, dir) = locate(app, Some(name))?;
    if let Some(r) = dir.runs().iter().find(|r| r.is_running()) {
        return Err(AgentError::RunBusy {
            run: r.n,
            task: name.to_string(),
        }
        .into());
    }
    let mut run = dir.read_run(n)?.ok_or(AgentError::RunNotFound(n))?;
    if !run.is_queued() {
        return Err(AgentError::RunNotQueued {
            run: n,
            status: run.shown_status().to_string(),
        }
        .into());
    }
    let fail = |mut run: RunRecord, e: IterError| {
        run.status = Status::Failed;
        run.ended = Some(now());
        let _ = std::fs::write(dir.run_log_path(n), format!("iter: {e}\n"));
        let _ = dir.write_run(&run);
        e
    };
    let kind = run.kind;
    // read-only runs need no branch; code-changing ones get the task's workspace first,
    // so that the prompt names the right directory
    let placed = (|| -> Result<(Option<String>, Option<String>)> {
        let (branch, worktree) = if kind.changes_code() {
            workspace(app, &project, &mut task)?
        } else {
            app.db
                .find_session_config_by_task(task.id())?
                .map_or((None, None), |c| (c.github_branch, c.worktree_path))
        };
        let at = worktree.as_deref().unwrap_or(&project.base_path);
        let text = std::fs::read_to_string(dir.run_prompt_path(n))?;
        std::fs::write(dir.run_prompt_path(n), text.replace("{{worktree}}", at))?;
        Ok((branch, worktree))
    })();
    match placed {
        Ok((branch, worktree)) => (run.branch, run.worktree) = (branch, worktree),
        Err(e) => return Err(fail(run, e)),
    }
    if let Err(e) = start_work(app, &mut task) {
        return Err(fail(run, e));
    }
    if dir.runs().iter().all(|r| r.n == n || r.is_queued()) {
        say.ingest(&project, &auto(app, &project, &task, Trigger::TaskStart));
    }
    run.status = Status::Running;
    run.started = now();
    run.pid = wait.then(std::process::id);
    // whatever goes wrong from here on is recorded, not left `running`
    let launched = (|| -> Result<()> {
        dir.write_run(&run)?;
        if wait {
            say.working(name, &run);
            work(&project, &dir, run.clone(), extra)?;
            return Ok(());
        }
        let pid = exec::spawn_detached(
            std::process::Command::new(std::env::current_exe()?)
                .args(["task", "run-worker", &task.id().to_string(), &n.to_string()])
                .arg("--")
                .args(extra),
        )?;
        // the worker writes its own record; only fill in its pid if it has not yet
        if let Some(mut current) = dir.read_run(n)?
            && current.status == Status::Running
            && current.pid.is_none()
        {
            current.pid = Some(pid);
            dir.write_run(&current)?;
        }
        Ok(())
    })();
    launched.map_err(|e| fail(run, e))?;
    Ok(())
}

/// Stops run `n`: a waiting one is dropped, a working one is told to stop; both end `cancelled`.
#[cfg(feature = "web")]
pub fn cancel(app: &App, name: &str, n: u32) -> Result<()> {
    let (_, _, dir) = locate(app, Some(name))?;
    let mut run = dir.read_run(n)?.ok_or(AgentError::RunNotFound(n))?;
    let working = run.is_running();
    if !working && !run.is_queued() {
        return Err(AgentError::RunNotStoppable {
            run: n,
            status: run.shown_status().to_string(),
        }
        .into());
    }
    // the record first: the worker, when it wakes, leaves a cancelled run as it is
    let pid = run.pid.filter(|_| working);
    run.status = Status::Cancelled;
    run.ended = Some(now());
    dir.write_run(&run)?;
    if let Some(pid) = pid {
        crate::agent::run::interrupt(pid);
    }
    Ok(())
}

/// Starts the harness of `run` and waits for it; output goes to the run's log.
fn launch(
    project: &Project,
    dir: &TaskDir,
    run: &RunRecord,
    extra: &[String],
) -> Result<Option<i32>> {
    let prompt = std::fs::read_to_string(dir.run_prompt_path(run.n))?;
    let log = std::fs::File::create(dir.run_log_path(run.n))?;
    let (program, args) = run.agent.command(&Invocation {
        prompt: &prompt,
        model: &run.model,
        effort: &run.effort,
        dirs: &[dir.root()],
        allowed: &crate::config::config().agent.allowed_tools,
        extra,
    });
    let cwd = run.worktree.as_deref().unwrap_or(&project.base_path);
    let status = exec::run_logged(program, &args, Path::new(cwd), log)?;
    Ok(exec::exit_code(status))
}

/// Runs the harness to the end and records the outcome; a run that cannot start is failed, reason in its log.
fn work(
    project: &Project,
    dir: &TaskDir,
    mut run: RunRecord,
    extra: &[String],
) -> Result<RunRecord> {
    let outcome = launch(project, dir, &run, extra);
    // an agent that wrote the plan itself leaves the steps without ids: number them now
    let _ = dir.update_plan(|text| {
        Ok((
            text.and_then(|t| {
                let numbered = crate::agent::plan::normalize(&t);
                (numbered != t).then_some(numbered)
            }),
            (),
        ))
    });
    run.ended = Some(now());
    match outcome {
        Ok(code) => {
            run.exit_code = code;
            run.status = if code == Some(0) {
                Status::Done
            } else {
                Status::Failed
            };
        }
        Err(e) => {
            let _ = std::fs::write(dir.run_log_path(run.n), format!("iter: {e}\n"));
            run.status = Status::Failed;
        }
    }
    // stopped by the user while the harness was winding down: it stays stopped
    if dir
        .read_run(run.n)?
        .is_some_and(|r| r.status == Status::Cancelled)
    {
        return Ok(run);
    }
    dir.write_run(&run)?;
    Ok(run)
}

pub fn worker(app: &App, task_id: i64, n: u32, extra: &[String]) -> Result<()> {
    let task = app.db.get::<Task>(task_id)?.ok_or(AgentError::TaskGone)?;
    let project = app
        .db
        .get::<Project>(task.project_id)?
        .ok_or(AgentError::ProjectGone)?;
    let dir = crate::agent::taskdir::of(&app.db, &project, &task)?;
    let mut run = dir.read_run(n)?.ok_or(AgentError::RunNotFound(n))?;
    run.pid = Some(std::process::id());
    dir.write_run(&run)?;
    work(&project, &dir, run, extra).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world(name: &str) -> App {
        App::with_db(crate::fixtures::world(&format!("solve-{name}")).db)
    }

    fn spec(model: &str) -> Spec {
        Spec {
            agent: Harness::Claude,
            model: model.into(),
            effort: "low".into(),
        }
    }

    #[cfg(feature = "web")]
    #[test]
    fn a_run_is_written_down_edited_and_taken_out_of_the_line_before_it_begins() {
        let app = world("queue");
        let name = "app/Fix it";
        let n = create(
            &app,
            name,
            Kind::Develop,
            &spec("haiku"),
            "Retries.",
            None,
            None,
        )
        .unwrap();
        let (_, _, dir) = locate(&app, Some(name)).unwrap();
        let run = dir.read_run(n).unwrap().unwrap();
        assert!(run.is_queued() && run.pid.is_none());
        let told = std::fs::read_to_string(dir.run_prompt_path(n)).unwrap();
        assert!(told.contains("Retries.") && told.contains("{{worktree}}"));

        amend(
            &app,
            name,
            n,
            Kind::Fix,
            "fix",
            &spec("opus"),
            "Do it my way",
        )
        .unwrap();
        let run = dir.read_run(n).unwrap().unwrap();
        assert_eq!((run.kind, run.model.as_str()), (Kind::Fix, "opus"));
        assert_eq!(
            std::fs::read_to_string(dir.run_prompt_path(n)).unwrap(),
            "Do it my way"
        );

        let m = create(
            &app,
            name,
            Kind::Plan,
            &spec("haiku"),
            "",
            Some("Mine"),
            None,
        )
        .unwrap();
        assert_eq!(m, n + 1);
        assert_eq!(
            std::fs::read_to_string(dir.run_prompt_path(m)).unwrap(),
            "Mine"
        );

        cancel(&app, name, n).unwrap();
        let run = dir.read_run(n).unwrap().unwrap();
        assert_eq!((run.status, run.ended.is_some()), (Status::Cancelled, true));
        assert!(amend(&app, name, n, Kind::Fix, "fix", &spec("opus"), "x").is_err());
        assert!(begin(&app, name, n, &Launch::detached()).is_err());
        assert!(cancel(&app, name, n).is_err());
        assert!(!dir.read_run(m).unwrap().unwrap().is_running());
    }

    #[test]
    fn an_agent_starting_makes_a_queued_task_work_in_progress_and_leaves_a_done_one() {
        for (before, after) in [
            (TaskStatus::Queue, TaskStatus::Wip),
            (TaskStatus::Wip, TaskStatus::Wip),
            (TaskStatus::Done, TaskStatus::Done),
        ] {
            let app = world(&format!("wip-{}", before.as_str()));
            let (_, mut task, _) = locate(&app, Some("app/Fix it")).unwrap();
            task.status = before;
            app.db.update(task.id(), &task).unwrap();
            start_work(&app, &mut task).unwrap();
            let (_, stored, _) = locate(&app, Some("app/Fix it")).unwrap();
            assert_eq!((task.status, stored.status), (after, after));
        }
    }
}
