//! `iter task solve`, `iter task runs`: handing a task to an agent that works
//! on its own in the task's worktree. The files of a run are
//! `agent::run`'s; the harness is whatever `Harness` says to start.

use crate::agent::prompt::Kind;
use crate::agent::run::{Harness, Run, Status};
use crate::agent::taskdir::TaskDir;
use crate::app::App;
use crate::commands::agent::{locate, prompt_for};
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
    task.status = TaskStatus::Wip;
    db.update(task.id(), task)?;
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
/// as the task: makes the workspace if there is none, writes the prompt and
/// the record, and starts the harness, detached unless `wait`. The number of
/// the run.
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
    let prompt = prompt_for(app, Some(name), kind, None, true, Some(description))?;
    let prompt = format!("{prompt}\n\nWork at {} effort.\n", spec.effort.trim());
    let (branch, worktree) = workspace(app, &project, &mut task)?;

    let n = dir.next_run();
    dir.ensure()?;
    std::fs::create_dir_all(dir.runs_dir())?;
    std::fs::write(dir.run_prompt_path(n), &prompt)?;
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
        pid: None,
    };
    dir.write_run(&run)?;

    if wait {
        println!("run {n} of {name}: {} working...", run.agent.key());
        work(&project, &dir, run, extra)?;
        return Ok(n);
    }
    let child = std::process::Command::new(std::env::current_exe()?)
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
    run.pid = Some(child.id());
    dir.write_run(&run)?;
    Ok(n)
}

/// Runs the harness of `run` in its worktree to the end, its output in the
/// run's log, and records how it went.
fn work(project: &Project, dir: &TaskDir, mut run: Run, extra: &[String]) -> Result<Run> {
    let prompt = std::fs::read_to_string(dir.run_prompt_path(run.n))?;
    let log = std::fs::File::create(dir.run_log_path(run.n))?;
    let (program, args) = run.agent.command(&run.model, &prompt, extra);
    let cwd = run.worktree.as_deref().unwrap_or(&project.base_path);
    let outcome = std::process::Command::new(program)
        .args(&args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .status();
    run.ended = Some(now());
    match outcome {
        Ok(status) => {
            run.exit_code = status.code();
            run.status = if status.success() {
                Status::Done
            } else {
                Status::Failed
            };
        }
        Err(e) => {
            let _ = std::fs::write(
                dir.run_log_path(run.n),
                format!("cannot start {program}: {e}\n"),
            );
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
