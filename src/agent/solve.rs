//! The life of an agent run: `create`, `amend`, `begin`, `cancel` and the detached `worker`; shared by `iter task solve` and the web page.
//! Callers hear what happens through an [`Observer`]; nothing prints here.

use crate::agent::exec;
use crate::agent::run::{Harness, Invocation, RunRecord, Status};
use crate::agent::task_prompt::{fill, instruction_for, locate};
use crate::agent::taskdir::TaskDir;
use crate::app::App;
use crate::db::Table;
use crate::error::AgentError;
use crate::error::{IterError, Result};
use crate::git;
use crate::models::{Project, SessionConfig, Task, TaskStatus};
use crate::text::now_as;
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
pub(super) struct Quiet;

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
    now_as("%Y-%m-%d %H:%M:%S")
}

/// Starts a run of the instruction `instruction` on `name` (`create` then `begin`), detached unless `wait`; returns its number. One run at a time per task.
pub fn start(
    app: &App,
    name: &str,
    instruction: &str,
    spec: &Spec,
    description: &str,
    launch: &Launch,
) -> Result<u32> {
    let (_, _, dir) = locate(app, Some(name))?;
    not_busy(&dir, name)?;
    let n = create(app, name, instruction, spec, description, None)?;
    begin(app, name, n, launch)?;
    Ok(n)
}

/// Writes a run of the instruction `instruction` down without starting it (`queued`): its prompt
/// is `prompt`, else the instruction's filled in. An interactive instruction is refused, and so is
/// one found nowhere. `{{worktree}}` in the prompt is resolved at `begin`.
pub fn create(
    app: &App,
    name: &str,
    instruction: &str,
    spec: &Spec,
    description: &str,
    prompt: Option<&str>,
) -> Result<u32> {
    let (project, task, dir) = locate(app, Some(name))?;
    let found = instruction_for(app, &project, &task, instruction)?;
    if found.front.interactive {
        return Err(AgentError::Interactive(instruction.to_string()).into());
    }
    dir.ensure()?;
    let text = match prompt {
        Some(text) if !text.trim().is_empty() => text.to_string(),
        _ => fill(
            app,
            (&project, &task, &dir),
            instruction,
            &found.body,
            true,
            Some(description),
            true,
        )?,
    };
    let n = dir.claim_run()?;
    let run = RunRecord {
        n,
        agent: spec.agent,
        model: spec.model.clone(),
        effort: spec.effort.clone(),
        instruction: instruction.to_string(),
        wants_worktree: found.front.worktree,
        status: Status::Queued,
        started: now(),
        ended: None,
        exit_code: None,
        branch: None,
        worktree: None,
        pid: None,
        subagent: false,
    };
    let written = crate::files::write_private(&dir.run_prompt_path(n), text)
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
    instruction: &str,
    spec: &Spec,
    prompt: &str,
) -> Result<()> {
    let (project, task, dir) = locate(app, Some(name))?;
    let mut run = dir.read_run(n)?.ok_or(AgentError::RunNotFound(n))?;
    if !run.is_queued() {
        return Err(AgentError::RunBegun(n).into());
    }
    let found = instruction_for(app, &project, &task, instruction)?;
    if found.front.interactive {
        return Err(AgentError::Interactive(instruction.to_string()).into());
    }
    run.agent = spec.agent;
    run.model = spec.model.clone();
    run.effort = spec.effort.clone();
    run.instruction = instruction.to_string();
    run.wants_worktree = found.front.worktree;
    crate::files::write_private(&dir.run_prompt_path(n), prompt)?;
    dir.write_run(&run)?;
    Ok(())
}

/// A queued run made ready to work: in its workspace, the task in progress, marked running.
struct Ready {
    project: Project,
    task: Task,
    dir: TaskDir,
    run: RunRecord,
}

/// Records `run` as failed for `e`, the reason in its log.
fn fail(dir: &TaskDir, mut run: RunRecord, e: IterError) -> IterError {
    run.status = Status::Failed;
    run.ended = Some(now());
    let _ = crate::files::write_private(&dir.run_log_path(run.n), format!("iter: {e}\n"));
    let _ = dir.write_run(&run);
    e
}

/// Refuses while another run of the task is working.
fn not_busy(dir: &TaskDir, name: &str) -> Result<()> {
    match dir.runs().iter().find(|r| r.is_running()) {
        Some(r) => Err(AgentError::RunBusy {
            run: r.n,
            task: name.to_string(),
        }
        .into()),
        None => Ok(()),
    }
}

/// What every run does before its agent starts: makes the workspace if the instruction asks for it
/// (before the prompt is final, so it names the right directory), marks the task in progress,
/// may trigger an ingest, then `running`. The record is not written; a failure is recorded.
fn prepare(app: &App, name: &str, n: u32, say: &dyn Observer) -> Result<Ready> {
    let (project, mut task, dir) = locate(app, Some(name))?;
    not_busy(&dir, name)?;
    let mut run = dir.read_run(n)?.ok_or(AgentError::RunNotFound(n))?;
    if !run.is_queued() {
        return Err(AgentError::RunNotQueued {
            run: n,
            status: run.shown_status().to_string(),
        }
        .into());
    }
    // read-only runs need no branch; code-changing ones (`worktree: true`) get the task's
    // workspace first, so that the prompt names the right directory
    let placed = (|| -> Result<(Option<String>, Option<String>)> {
        let (branch, worktree) = if run.wants_worktree {
            workspace(app, &project, &mut task)?
        } else {
            app.db
                .find_session_config_by_task(task.id())?
                .map_or((None, None), |c| (c.github_branch, c.worktree_path))
        };
        let at = worktree.as_deref().unwrap_or(&project.base_path);
        let text = std::fs::read_to_string(dir.run_prompt_path(n))?;
        crate::files::write_private(&dir.run_prompt_path(n), text.replace("{{worktree}}", at))?;
        Ok((branch, worktree))
    })();
    match placed {
        Ok((branch, worktree)) => (run.branch, run.worktree) = (branch, worktree),
        Err(e) => return Err(fail(&dir, run, e)),
    }
    if let Err(e) = start_work(app, &mut task) {
        return Err(fail(&dir, run, e));
    }
    if dir.runs().iter().all(|r| r.n == n || r.is_queued()) {
        say.ingest(&project, &auto(app, &project, &task, Trigger::TaskStart));
    }
    run.status = Status::Running;
    run.started = now();
    Ok(Ready {
        project,
        task,
        dir,
        run,
    })
}

/// Begins queued run `n`: makes the workspace if its instruction asks for one, starts the harness (detached unless `wait`); a run that cannot begin is recorded as failed.
pub fn begin(app: &App, name: &str, n: u32, launch: &Launch) -> Result<()> {
    let Launch { wait, extra, say } = *launch;
    let Ready {
        project,
        task,
        dir,
        mut run,
    } = prepare(app, name, n, say)?;
    run.pid = wait.then(std::process::id);
    // whatever goes wrong from here on is recorded, not left `running`
    let launched = (|| -> Result<()> {
        dir.write_run(&run)?;
        if wait {
            say.working(name, &run);
            work(&project, task.id(), &dir, run.clone(), extra)?;
            return Ok(());
        }
        let pid = exec::spawn_detached(
            std::process::Command::new(crate::process::host()?).args(worker_args(
                task.id(),
                n,
                extra,
            )),
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
    launched.map_err(|e| fail(&dir, run, e))?;
    Ok(())
}

/// Opens a run of the instruction `instruction` for a subagent of the session the caller is in: iter starts nothing,
/// the session gives the subagent the prompt and closes the run with [`close`]. Returns the run's
/// number and its prompt, final (the worktree named). `spec` is what the subagent is started with.
pub fn open_subagent(
    app: &App,
    name: &str,
    instruction: &str,
    spec: &Spec,
    say: &dyn Observer,
) -> Result<(u32, String)> {
    let (_, task, dir) = locate(app, Some(name))?;
    not_busy(&dir, name)?;
    let n = create(app, name, instruction, spec, &task.description, None)?;
    let Ready { dir, mut run, .. } = prepare(app, name, n, say)?;
    run.subagent = true;
    let prompt = (|| -> Result<String> {
        dir.write_run(&run)?;
        Ok(std::fs::read_to_string(dir.run_prompt_path(n))?)
    })();
    let prompt = prompt.map_err(|e| fail(&dir, run, e))?;
    Ok((n, prompt))
}

/// Closes subagent run `n` as `status` (done, failed or cancelled), keeping `report` (what the
/// subagent said at the end) as its log. A run iter started closes itself and is refused.
pub fn close(app: &App, name: &str, n: u32, status: Status, report: &str) -> Result<()> {
    let (_, _, dir) = locate(app, Some(name))?;
    let mut run = dir.read_run(n)?.ok_or(AgentError::RunNotFound(n))?;
    if !run.subagent {
        return Err(AgentError::RunNotSubagent(n).into());
    }
    if run.status != Status::Running || matches!(status, Status::Queued | Status::Running) {
        return Err(AgentError::RunNotOpen {
            run: n,
            status: run.shown_status().to_string(),
        }
        .into());
    }
    let mut log = report.trim_end().to_string();
    log.push('\n');
    crate::files::write_private(&dir.run_log_path(n), log)?;
    run.status = status;
    run.ended = Some(now());
    dir.write_run(&run)?;
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

/// The one folder of the task's an agent may edit besides its worktree: its sketches. Not the task
/// folder itself, which holds `runs/`: a run record the agent could rewrite would decide what Stop
/// signals and which worktree is torn down. The plan goes through `iter task plan`.
fn agent_dir(dir: &TaskDir) -> std::path::PathBuf {
    dir.sketches_dir()
}

/// Starts the harness of `run` and waits for it; output goes to the run's log.
fn launch(
    project: &Project,
    task_id: i64,
    dir: &TaskDir,
    run: &RunRecord,
    extra: &[String],
) -> Result<Option<i32>> {
    let prompt = std::fs::read_to_string(dir.run_prompt_path(run.n))?;
    let log = crate::files::create_private(&dir.run_log_path(run.n))?;
    let writable = agent_dir(dir);
    let (program, args) = run.agent.command(&Invocation {
        model: &run.model,
        effort: &run.effort,
        dirs: &[writable.as_path()],
        allowed: &crate::config::config().agent.allowed_tools,
        extra,
    });
    let cwd = run.worktree.as_deref().unwrap_or(&project.base_path);
    let env = [
        (crate::locate::ENV_TASK, task_id.to_string()),
        (crate::locate::ENV_PROJECT, project.id().to_string()),
    ];
    let status = exec::run_logged(program, &args, Path::new(cwd), &env, &prompt, log)?;
    Ok(exec::exit_code(status))
}

/// Runs the harness to the end and records the outcome; a run that cannot start is failed, reason in its log.
fn work(
    project: &Project,
    task_id: i64,
    dir: &TaskDir,
    mut run: RunRecord,
    extra: &[String],
) -> Result<RunRecord> {
    let outcome = launch(project, task_id, dir, &run, extra);
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
            let _ = crate::files::write_private(&dir.run_log_path(run.n), format!("iter: {e}\n"));
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
    work(&project, task.id(), &dir, run, extra).map(|_| ())
}

/// The arguments of the detached worker: `iter task run-worker <task> <run> -- <extra..>`.
fn worker_args(task_id: i64, run: u32, extra: &[String]) -> Vec<String> {
    ["task", "run-worker"]
        .map(String::from)
        .into_iter()
        .chain([task_id.to_string(), run.to_string(), "--".to_string()])
        .chain(extra.iter().cloned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The command line a detached worker is started with must parse as `iter task run-worker`:
    /// rename the subcommand and this fails, instead of a worker that exits 2 unseen.
    #[test]
    fn the_worker_command_line_parses_as_run_worker() {
        use crate::args::{Args, Command, TaskCommand};
        use clap::Parser;
        for extra in [vec![], vec!["--model".to_string(), "-x".to_string()]] {
            let mut argv = vec!["iter".to_string()];
            argv.extend(worker_args(42, 3, &extra));
            let parsed = Args::try_parse_from(&argv).unwrap_or_else(|e| panic!("{argv:?}: {e}"));
            match parsed.command {
                Command::Task {
                    action:
                        Some(TaskCommand::RunWorker {
                            task_id,
                            run,
                            extra: passed,
                        }),
                } => assert_eq!((task_id, run, passed), (42, 3, extra)),
                other => panic!("{argv:?} parsed as {other:?}"),
            }
        }
    }

    #[test]
    fn the_agent_cannot_reach_the_run_records() {
        let dir = TaskDir::new("/notes/tasks/7".into());
        let writable = agent_dir(&dir);
        assert!(!dir.runs_dir().starts_with(&writable));
        assert!(!dir.plan_path().starts_with(&writable));
        assert!(writable.starts_with(dir.root()));
    }

    /// The agent test world, its notes holding the shipped instructions.
    fn world(name: &str) -> App {
        let w = crate::fixtures::world(&format!("solve-{name}"));
        crate::fixtures::install_instructions(&w.notes);
        App::with_db(w.db)
    }

    #[cfg(feature = "web")]
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
        let n = create(&app, name, "develop", &spec("haiku"), "Retries.", None).unwrap();
        let (_, _, dir) = locate(&app, Some(name)).unwrap();
        let run = dir.read_run(n).unwrap().unwrap();
        assert!(run.is_queued() && run.pid.is_none());
        let told = std::fs::read_to_string(dir.run_prompt_path(n)).unwrap();
        assert!(told.contains("Retries.") && told.contains("{{worktree}}"));

        amend(&app, name, n, "fix", &spec("opus"), "Do it my way").unwrap();
        let run = dir.read_run(n).unwrap().unwrap();
        assert_eq!(
            (run.instruction.as_str(), run.model.as_str()),
            ("fix", "opus")
        );
        assert_eq!(
            std::fs::read_to_string(dir.run_prompt_path(n)).unwrap(),
            "Do it my way"
        );

        let m = create(&app, name, "plan", &spec("haiku"), "", Some("Mine")).unwrap();
        assert_eq!(m, n + 1);
        assert_eq!(
            std::fs::read_to_string(dir.run_prompt_path(m)).unwrap(),
            "Mine"
        );

        cancel(&app, name, n).unwrap();
        let run = dir.read_run(n).unwrap().unwrap();
        assert_eq!((run.status, run.ended.is_some()), (Status::Cancelled, true));
        assert!(amend(&app, name, n, "fix", &spec("opus"), "x").is_err());
        assert!(begin(&app, name, n, &Launch::detached()).is_err());
        assert!(cancel(&app, name, n).is_err());
        assert!(!dir.read_run(m).unwrap().unwrap().is_running());
    }

    #[test]
    fn a_subagent_run_is_opened_with_its_prompt_and_closed_by_the_session() {
        let app = world("subagent");
        let name = "app/Fix it";
        let sonnet = Spec {
            agent: Harness::Claude,
            model: "sonnet".into(),
            effort: "high".into(),
        };
        let (n, prompt) = open_subagent(&app, name, "test", &sonnet, &Quietly).unwrap();
        assert!(prompt.contains("Retries hammer the api.") && !prompt.contains("{{"));
        let (_, task, dir) = locate(&app, Some(name)).unwrap();
        assert_eq!(task.status, TaskStatus::Wip);
        let run = dir.read_run(n).unwrap().unwrap();
        assert!(run.subagent && run.pid.is_none() && run.is_running());
        assert_eq!(
            (run.model.as_str(), run.effort.as_str()),
            ("sonnet", "high")
        );
        assert_eq!(
            std::fs::read_to_string(dir.run_prompt_path(n)).unwrap(),
            prompt
        );

        // one at a time, and nothing written down for the refused one
        let busy = open_subagent(&app, name, "develop", &sonnet, &Quietly).unwrap_err();
        assert!(busy.to_string().contains("still working"), "{busy}");
        assert_eq!(dir.runs().len(), 1);
        assert!(close(&app, name, n, Status::Running, "x").is_err());

        close(&app, name, n, Status::Done, "R1 tested\n\n").unwrap();
        let run = dir.read_run(n).unwrap().unwrap();
        assert_eq!((run.status, run.ended.is_some()), (Status::Done, true));
        assert_eq!(
            std::fs::read_to_string(dir.run_log_path(n)).unwrap(),
            "R1 tested\n"
        );
        let again = close(&app, name, n, Status::Failed, "").unwrap_err();
        assert!(again.to_string().contains("is done"), "{again}");

        let (m, _) = open_subagent(&app, name, "check", &sonnet, &Quietly).unwrap();
        assert_eq!(m, n + 1);
    }

    #[test]
    fn a_run_iter_started_is_not_closed_by_a_session_and_complete_is_never_a_run() {
        let app = world("not-subagent");
        let name = "app/Fix it";
        let spec = Spec {
            agent: Harness::Claude,
            model: "haiku".into(),
            effort: "low".into(),
        };
        let n = create(&app, name, "develop", &spec, "x", None).unwrap();
        let e = close(&app, name, n, Status::Done, "").unwrap_err();
        assert!(
            matches!(e, IterError::Agent(AgentError::RunNotSubagent(_))),
            "{e}"
        );
        assert!(close(&app, name, 99, Status::Done, "").is_err());

        let e = create(&app, name, "complete", &spec, "x", None).unwrap_err();
        assert!(
            e.to_string().contains("iter task prompt -i complete"),
            "{e}"
        );
        assert!(open_subagent(&app, name, "complete", &spec, &Quietly).is_err());
        let (_, _, dir) = locate(&app, Some(name)).unwrap();
        assert_eq!(dir.runs().len(), 1);
    }

    /// R2: the ingest of a closing task is a subagent run like the others, opened and closed.
    #[test]
    fn an_ingest_subagent_run_is_opened_and_closed() {
        let app = world("ingest");
        let name = "app/Fix it";
        let spec = Spec {
            agent: Harness::Claude,
            model: "haiku".into(),
            effort: "low".into(),
        };
        let (n, prompt) = open_subagent(&app, name, "ingest", &spec, &Quietly).unwrap();
        assert!(!prompt.contains("{{"), "{prompt}");
        let (_, _, dir) = locate(&app, Some(name)).unwrap();
        let run = dir.read_run(n).unwrap().unwrap();
        assert!(run.subagent && run.is_running() && run.instruction == "ingest");
        close(&app, name, n, Status::Done, "2 pages updated").unwrap();
        assert_eq!(dir.read_run(n).unwrap().unwrap().status, Status::Done);
    }

    struct Quietly;
    impl Observer for Quietly {}

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

    /// Characterisation (R7, `now()`): a run's stamp is the local time as `YYYY-MM-DD HH:MM:SS`.
    #[test]
    fn a_run_is_stamped_with_the_local_time_and_a_space() {
        let stamp = now();
        assert_eq!(stamp.as_bytes()[10], b' ', "{stamp}");
        let at = chrono::NaiveDateTime::parse_from_str(&stamp, "%Y-%m-%d %H:%M:%S").unwrap();
        let off = (chrono::Local::now().naive_local() - at)
            .num_seconds()
            .abs();
        assert!(off < 5, "{stamp} is {off}s from now");
    }
}
