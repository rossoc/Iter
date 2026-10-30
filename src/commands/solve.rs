//! `iter task solve`, `iter task runs`: handing a task to an agent that works
//! on its own in the task's worktree. The files of a run are
//! `agent::run`'s; the harness is whatever `Harness` says to start.

use crate::agent::prompt::Kind;
use crate::agent::run::{Harness, Run, Status};
use crate::agent::taskdir::TaskDir;
use crate::app::App;
use crate::commands::agent::{locate, prompt_for};
use crate::commands::ingest::{self, Trigger};
use crate::db::Table;
use crate::error::{IterError, Result};
use crate::git;
use crate::md_edit::{edit_text, split_at_divider};
use crate::models::{Project, SessionConfig, Task, TaskStatus};
use crate::utils::resolve::task_display;
use crate::utils::workspace::create_task_worktree;
use serde::{Deserialize, Serialize};
use std::os::unix::process::CommandExt;
use std::process::Stdio;

fn failed(message: impl Into<String>) -> IterError {
    IterError::CommandFailed(message.into())
}

/// What a run is told to be -- the harness, the model, how hard to work --
/// and the front matter of the buffer `solve` opens.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Spec {
    pub agent: Harness,
    pub model: String,
    pub effort: String,
}

pub struct SolveRequest<'a> {
    pub task: Option<&'a str>,
    pub kind: Kind,
    pub agent: Option<Harness>,
    pub model: Option<&'a str>,
    pub effort: Option<&'a str>,
    pub yes: bool,
    pub wait: bool,
    pub extra: &'a [String],
}

/// What the buffer holds: the front matter and, below it, the description the
/// agent is given.
fn render(spec: &Spec, description: &str) -> Result<String> {
    Ok(format!(
        "---\n{}---\n\n{}\n",
        serde_yaml::to_string(spec)?,
        description.trim_end()
    ))
}

fn parse(buffer: &str) -> Result<(Spec, String)> {
    let buffer = buffer.strip_prefix("---\n").unwrap_or(buffer);
    let (front, body) = split_at_divider(buffer)
        .ok_or_else(|| failed("the buffer lost its front matter (the lines between `---`)"))?;
    let spec = serde_yaml::from_str(front).map_err(|e| failed(format!("front matter: {e}")))?;
    Ok((spec, body.trim().to_string()))
}

/// The worktree the run happens in: the task's own when it has one, else a
/// new one (with the branch), recorded so `task done` finds it. `(branch,
/// path)`; neither when the project does not use branches.
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
        return Err(failed(format!(
            "{} is not a git repository",
            project.base_path
        )));
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
    // a task that is done or already going keeps its status
    if task.status == TaskStatus::Queue {
        task.status = TaskStatus::Wip;
        db.update(task.id(), task)?;
    }
    Ok((Some(branch), Some(path)))
}

fn now() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

pub fn solve(app: &App, req: &SolveRequest) -> Result<()> {
    let (project, task, dir) = locate(app, req.task)?;
    let name = task_display(&project, &task);
    let mut spec = Spec {
        agent: req.agent.unwrap_or_default(),
        model: req.model.unwrap_or("haiku").to_string(),
        effort: req.effort.unwrap_or("medium").to_string(),
    };
    let mut description = task.description.clone();
    if !req.yes {
        match edit_text(&render(&spec, &description)?)? {
            Some(text) => (spec, description) = parse(&text)?,
            None => {
                println!("no changes -- nothing started (save the buffer to start, or use --yes)");
                return Ok(());
            }
        }
    }
    let n = start(
        app,
        &name,
        req.kind,
        &spec,
        &description,
        req.wait,
        req.extra,
    )?;
    if req.wait {
        let run = dir.read_run(n)?.ok_or_else(|| failed("the run vanished"))?;
        println!("run {n}: {}", run.shown_status());
    } else {
        println!(
            "run {n} of {name} started ({}). Follow it: iter task runs {} --log {n}",
            spec.agent.key(),
            crate::agent::prompt::shell_word(&name)
        );
    }
    Ok(())
}

/// Starts a run of `kind` on `name` (`<project>/<task>`) with `description`
/// as the task: makes the workspace if the kind changes code and there is
/// none, writes the prompt and the record, and starts the harness, detached
/// unless `wait`. The number of the run. One run at a time per task.
pub fn start(
    app: &App,
    name: &str,
    kind: Kind,
    spec: &Spec,
    description: &str,
    wait: bool,
    extra: &[String],
) -> Result<u32> {
    let (project, mut task, dir) = locate(app, Some(name))?;
    dir.ensure()?;
    if let Some(r) = dir.runs().iter().find(|r| r.is_running()) {
        return Err(failed(format!(
            "run {} is still working on '{name}': wait for it, or stop it first",
            r.n
        )));
    }
    // a run that only reads has no use for a branch of its own; one that
    // changes code works where the task's workspace is, made now if need be.
    // Before the prompt, so that it names the right directory.
    let (branch, worktree) = match kind {
        Kind::Execute | Kind::Fix => workspace(app, &project, &mut task)?,
        Kind::Plan | Kind::Investigate => app
            .db
            .find_session_config_by_task(task.id())?
            .map_or((None, None), |c| (c.github_branch, c.worktree_path)),
    };
    let prompt = prompt_for(app, Some(name), kind, None, true, Some(description))?;

    let n = dir.claim_run()?;
    if n == 1 {
        ingest::auto(app, &project, &task, Trigger::TaskStart);
    }
    let mut run = Run {
        n,
        agent: spec.agent,
        model: spec.model.clone(),
        effort: spec.effort.clone(),
        kind: kind.key().to_string(),
        status: Status::Running,
        started: now(),
        ended: None,
        exit_code: None,
        branch,
        worktree,
        pid: wait.then(std::process::id),
    };
    // whatever goes wrong from here on, the record says so rather than
    // staying `running` for good
    let launched = (|| -> Result<()> {
        std::fs::write(dir.run_prompt_path(n), &prompt)?;
        dir.write_run(&run)?;
        if wait {
            println!("run {n} of {name}: {} working...", run.agent.key());
            work(&project, &dir, run.clone(), extra)?;
            return Ok(());
        }
        let mut child = std::process::Command::new(std::env::current_exe()?)
            .args(["task", "run-worker", &task.id().to_string(), &n.to_string()])
            .arg("--")
            .args(extra)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .map_err(|source| IterError::Spawn {
                tool: "iter",
                source,
            })?;
        let pid = child.id();
        // in the web server this process lives on: the worker is waited for,
        // so that it does not stay a zombie that still looks alive
        std::thread::spawn(move || {
            let _ = child.wait();
        });
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
    if let Err(e) = launched {
        run.status = Status::Failed;
        run.ended = Some(now());
        let _ = std::fs::write(dir.run_log_path(n), format!("iter: {e}\n"));
        let _ = dir.write_run(&run);
        return Err(e);
    }
    Ok(n)
}

/// Starts the harness of `run` and waits for it; its output goes to the
/// run's log.
fn launch(project: &Project, dir: &TaskDir, run: &Run, extra: &[String]) -> Result<Option<i32>> {
    let prompt = std::fs::read_to_string(dir.run_prompt_path(run.n))?;
    let log = std::fs::File::create(dir.run_log_path(run.n))?;
    let (program, args) = run.agent.command(
        &run.model,
        &run.effort,
        &prompt,
        &[dir.root()],
        &crate::config::config().agent.allowed_tools,
        extra,
    );
    let cwd = run.worktree.as_deref().unwrap_or(&project.base_path);
    let status = std::process::Command::new(program)
        .args(&args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .status()
        .map_err(|source| IterError::Spawn {
            tool: program,
            source,
        })?;
    Ok(status.success().then_some(0).or(status.code().or(Some(-1))))
}

/// Runs the harness of `run` in its worktree to the end and records how it
/// went. A run that cannot start is recorded as failed, with the reason in
/// its log.
fn work(project: &Project, dir: &TaskDir, mut run: Run, extra: &[String]) -> Result<Run> {
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
    dir.write_run(&run)?;
    Ok(run)
}

/// What a detached `solve` runs.
pub fn worker(app: &App, task_id: i64, n: u32, extra: &[String]) -> Result<()> {
    let task = app
        .db
        .get::<Task>(task_id)?
        .ok_or_else(|| failed("no such task"))?;
    let project = app
        .db
        .get::<Project>(task.project_id)?
        .ok_or_else(|| failed("no such project"))?;
    let dir = crate::agent::taskdir::of(&app.db, &project, &task)?;
    let mut run = dir.read_run(n)?.ok_or_else(|| failed("no such run"))?;
    run.pid = Some(std::process::id());
    dir.write_run(&run)?;
    work(&project, &dir, run, extra).map(|_| ())
}

pub fn runs(app: &App, task: Option<&str>, log: Option<u32>) -> Result<()> {
    let (project, task, dir) = locate(app, task)?;
    if let Some(n) = log {
        let text = std::fs::read_to_string(dir.run_log_path(n))
            .map_err(|_| failed(format!("run {n} has no log")))?;
        print!("{text}");
        return Ok(());
    }
    let runs = dir.runs();
    if runs.is_empty() {
        println!(
            "No runs of {} yet: `iter task solve {}`.",
            task_display(&project, &task),
            crate::agent::prompt::shell_word(&task_display(&project, &task))
        );
    }
    for r in runs {
        println!(
            "#{:<3} {:<8} {:<11} {:<6} {}{}",
            r.n,
            r.shown_status(),
            r.kind,
            r.model,
            r.started,
            r.ended.map(|e| format!(" -> {e}")).unwrap_or_default()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_buffer_round_trips_and_a_lost_front_matter_is_an_error() {
        let spec = Spec {
            agent: Harness::Claude,
            model: "sonnet".into(),
            effort: "high".into(),
        };
        let text = render(&spec, "Fix the retries.\n\nSecond paragraph.\n").unwrap();
        assert_eq!(
            parse(&text).unwrap(),
            (spec, "Fix the retries.\n\nSecond paragraph.".to_string())
        );
        assert!(parse("just text").is_err());
        assert!(parse("---\nagent: nobody\nmodel: x\neffort: y\n---\nbody").is_err());
    }
}
