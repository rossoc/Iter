//! `iter task solve`, `iter task runs`: handing a task to an agent that works
//! on its own in the task's worktree. The files of a run are
//! `agent::run`'s; the harness is whatever `Harness` says to start.

use crate::agent::exec;
use crate::agent::prompt::Kind;
use crate::agent::run::{Harness, Invocation, RunRecord, Status};
use crate::agent::taskdir::TaskDir;
use crate::app::App;
use crate::commands::agent::{locate, prompt_for};
use crate::commands::ingest::{self, Trigger};
use crate::db::Table;
use crate::agent::error::AgentError;
use crate::error::{IterError, Result};
use crate::git;
use crate::md_edit::{edit_text, split_at_divider};
use crate::models::{Project, SessionConfig, Task, TaskStatus};
use crate::utils::resolve::task_display;
use crate::utils::workspace::create_task_worktree;
use serde::{Deserialize, Serialize};
use std::path::Path;



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
        .ok_or(AgentError::BufferLostFront)?;
    let spec = serde_yaml::from_str(front).map_err(AgentError::BufferFront)?;
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
    // a task that is done or already going keeps its status
    if task.status == TaskStatus::Queue {
        task.status = TaskStatus::Wip;
        db.update(task.id(), task)?;
    }
    Ok((Some(branch), Some(path)))
}

/// A task an agent starts on is work in progress, if it was only queued (a
/// task that is done stays done).
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
        let run = dir.read_run(n)?.ok_or(AgentError::RunVanished(n))?;
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
/// as the task: writes it down ([`create`]) and begins it ([`begin`]),
/// detached unless `wait`. The number of the run. One run at a time per task.
pub fn start(
    app: &App,
    name: &str,
    kind: Kind,
    spec: &Spec,
    description: &str,
    wait: bool,
    extra: &[String],
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
    begin(app, name, n, wait, extra)?;
    Ok(n)
}

/// Writes a run down without starting it: its record (`queued`) and its
/// prompt, which is `prompt` or else the one `kind` makes for the task;
/// `instruction` is the name of the instruction it comes from. The
/// prompt may say `{{worktree}}`: where that is is known when the run begins.
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
        kind: kind.key().to_string(),
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

/// Changes what a queued run will do. Only a run that has not begun can be
/// edited.
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
    run.kind = kind.key().to_string();
    run.instruction = Some(instruction.to_string());
    std::fs::write(dir.run_prompt_path(n), prompt)?;
    dir.write_run(&run)?;
    Ok(())
}

/// Begins queued run `n` of `name`: makes the workspace if the kind changes
/// code and there is none, tells the prompt where it is, and starts the
/// harness, detached unless `wait`. A run that cannot begin is recorded as
/// failed.
pub fn begin(app: &App, name: &str, n: u32, wait: bool, extra: &[String]) -> Result<()> {
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
    let Ok(kind) = <Kind as clap::ValueEnum>::from_str(&run.kind, true) else {
        let e: IterError = AgentError::RunKindUnknown {
            run: n,
            kind: run.kind.clone(),
        }
        .into();
        return Err(fail(run, e));
    };
    // a run that only reads has no use for a branch of its own; one that
    // changes code works where the task's workspace is, made now if need be.
    // Before the prompt, so that it names the right directory.
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
    // an agent working on the task makes it work in progress (a done task stays done)
    if let Err(e) = start_work(app, &mut task) {
        return Err(fail(run, e));
    }
    if dir.runs().iter().all(|r| r.n == n || r.is_queued()) {
        ingest::auto(app, &project, &task, Trigger::TaskStart);
    }
    run.status = Status::Running;
    run.started = now();
    run.pid = wait.then(std::process::id);
    // whatever goes wrong from here on, the record says so rather than
    // staying `running` for good
    let launched = (|| -> Result<()> {
        dir.write_run(&run)?;
        if wait {
            println!("run {n} of {name}: {} working...", run.agent.key());
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

/// Stops run `n` of `name`: a waiting one is dropped, a working one is told
/// to stop. Either ends `cancelled`.
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

/// Starts the harness of `run` and waits for it; its output goes to the
/// run's log.
fn launch(project: &Project, dir: &TaskDir, run: &RunRecord, extra: &[String]) -> Result<Option<i32>> {
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

/// Runs the harness of `run` in its worktree to the end and records how it
/// went. A run that cannot start is recorded as failed, with the reason in
/// its log.
fn work(project: &Project, dir: &TaskDir, mut run: RunRecord, extra: &[String]) -> Result<RunRecord> {
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

/// What a detached `solve` runs.
pub fn worker(app: &App, task_id: i64, n: u32, extra: &[String]) -> Result<()> {
    let task = app
        .db
        .get::<Task>(task_id)?
        .ok_or(AgentError::TaskGone)?;
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

pub fn runs(app: &App, task: Option<&str>, log: Option<u32>) -> Result<()> {
    let (project, task, dir) = locate(app, task)?;
    if let Some(n) = log {
        let text = std::fs::read_to_string(dir.run_log_path(n))
            .map_err(|_| AgentError::RunLogMissing(n))?;
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
        // the prompt is the kind's, with the task in it and the worktree left to be told
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
        assert_eq!((run.kind.as_str(), run.model.as_str()), ("fix", "opus"));
        assert_eq!(
            std::fs::read_to_string(dir.run_prompt_path(n)).unwrap(),
            "Do it my way"
        );

        // a prompt of one's own is kept as it is
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
        // a run that is over cannot be edited, begun or stopped
        assert!(amend(&app, name, n, Kind::Fix, "fix", &spec("opus"), "x").is_err());
        assert!(begin(&app, name, n, false, &[]).is_err());
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
