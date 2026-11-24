//! `iter task` -- the CRUD and reporting half. The GitHub sync half is
//! [`super::sync`].

use crate::args::{ReportOpts, TaskCommand};
use crate::editor::edited;
use crate::ingest;
use crate::output::{print_yaml, report_header};
use crate::sync::{task_pull, task_push};
use crate::{Run, agent, solve};
use iter_core::agent::ingest_job::{self, Trigger};
use iter_core::app::App;
use iter_core::config::config;
use iter_core::db::{Db, Table};
use iter_core::error::{IterError, Result};
use iter_core::git;
use iter_core::github::{self, GhCli, IssueTracker};
use iter_core::locate::{project_or_current, require_git_repo, require_github, task_or_current};
use iter_core::models::require_name;
use iter_core::models::{Project, SessionConfig, Tag, Task, TaskEdit, TaskStatus};
use iter_core::reporting::{TaskInfo, WeekdayReport, print_report, session_rows, weekday_averages};
use iter_core::tmux::{Multiplexer, TmuxClient};
use iter_core::workspace::clock::close_open_session;
use iter_core::workspace::teardown_and_kill;

/// Bare `iter task` (unfinished work across projects) is handled in `run::run`: no `TaskCommand` variant expresses it.
impl Run for TaskCommand {
    fn run(&self, app: &App) -> Result<()> {
        match self {
            Self::New {
                project,
                name: Some(name),
                description,
                tags,
                status,
                ..
            } => task_named(
                app,
                project.as_deref(),
                name,
                description.as_deref(),
                tags,
                status.as_deref(),
            ),
            Self::New { project, issue, .. } => task_new(app, project.as_deref(), *issue),
            Self::Edit { task, tags, untags } if !tags.is_empty() || !untags.is_empty() => {
                task_retag(app, task.name(), tags, untags)
            }
            Self::Edit { task, .. } => task_edit(app, task.name()),
            Self::Delete { task } => task_delete(app, &TmuxClient::system(), task.name()),
            Self::Info { task, report } => task_info(app, task.name(), report),
            Self::List { project, status } => {
                let statuses = match status {
                    Some(s) => vec![
                        TaskStatus::parse(s)
                            .ok_or_else(|| IterError::InvalidStatus(s.to_string()))?,
                    ],
                    None => Vec::new(),
                };
                task_list(app, project.as_deref(), &statuses)
            }
            Self::Plan(a) => agent::plan(app, a),
            Self::Prompt(a) => agent::prompt_cmd(app, a),
            Self::Solve(a) if a.subagent => solve::subagent(app, a),
            Self::Solve(a) => solve::solve(app, a),
            Self::Runs(a) => solve::runs(app, a),
            Self::RunWorker {
                task_id,
                run,
                extra,
            } => iter_core::agent::solve::worker(app, *task_id, *run, extra),
            Self::Done { task, save } => task_done(app, &TmuxClient::system(), task.name(), *save),
            Self::Pull(a) => task_pull(
                app,
                &GhCli::system(),
                a.project.as_deref(),
                a.task.as_deref(),
                a.body,
            ),
            Self::Push(a) => task_push(
                app,
                &GhCli::system(),
                a.project.as_deref(),
                a.task.as_deref(),
                a.body,
            ),
            Self::Weekday { task } => task_weekday(app, task.name()),
        }
    }
}

fn task_new(app: &App, project_name: Option<&str>, issue: Option<i64>) -> Result<()> {
    let db = &app.db;
    let project = project_or_current(db, project_name)?;
    let project_id = project.id();

    let mut template = Task::template(
        project_id,
        git::branch_prefix(&project.branch_template),
        config().task.status,
    );
    if let Some(issue_number) = issue {
        require_github(&project)?;
        let issue = GhCli::system().fetch(&project.base_path, issue_number)?;
        template.name = issue.title;
        template.description = issue.body;
        template.github_issue = Some(issue.number);
        // a task opened from an already closed issue starts `done`, like `iter task pull`, so the next `push` does not reopen it
        template.status = github::status_for_issue_state(template.status, issue.state);
    }

    let template = TaskEdit {
        task: template,
        tags: Vec::new(),
        project: project.name.clone(),
    };
    edited(&template, "task", "created", |mut edit| {
        let tag_ids = checked_tags(db, &edit)?;
        let project = chosen_project(db, &project, &edit.project)?;
        edit.task.project_id = project.id();
        let id = db.insert(&edit.task)?;
        db.set_task_tags(id, &tag_ids)?;
        Ok(format!("created task '{}'", edit.task.label(&project)))
    })
}

/// `iter task new --name`: the task made at once, no editor (an agent's way, or a script's). The
/// description is `--description`, else standard input when something is piped. Tags that do not
/// exist are made, after the name is checked, so a refused task leaves no tag behind.
fn task_named(
    app: &App,
    project_name: Option<&str>,
    name: &str,
    description: Option<&str>,
    tags: &[String],
    status: Option<&str>,
) -> Result<()> {
    let db = &app.db;
    require_name("task", name)?;
    let project = project_or_current(db, project_name)?;
    let description = match description {
        Some(d) => d.to_string(),
        None => piped_text()?,
    };
    let status = match status {
        Some(s) => TaskStatus::parse(s).ok_or_else(|| IterError::InvalidStatus(s.to_string()))?,
        None => config().task.status,
    };
    let mut task = Task::template(
        project.id(),
        git::branch_prefix(&project.branch_template),
        status,
    );
    task.name = name.trim().to_string();
    task.description = description.trim_end().to_string();
    let tag_ids = ensure_tags(db, tags)?;
    let id = db.insert(&task)?;
    db.set_task_tags(id, &tag_ids)?;
    task.id = Some(id);
    println!("created task '{}'", task.label(&project));
    Ok(())
}

/// Standard input when it is not a terminal (an agent's or a pipe's), else nothing.
fn piped_text() -> Result<String> {
    use std::io::{IsTerminal, Read};
    let mut stdin = std::io::stdin();
    let mut text = String::new();
    if !stdin.is_terminal() {
        stdin.read_to_string(&mut text)?;
    }
    Ok(text)
}

/// The ids of the tags named `names`, making each one that does not exist. Every name is checked
/// before any tag is made.
fn ensure_tags(db: &Db, names: &[String]) -> Result<Vec<i64>> {
    for name in names {
        require_name("tag", name)?;
    }
    let mut ids = Vec::with_capacity(names.len());
    for name in names.iter().map(|n| n.trim()) {
        let id = match db.find_by_name::<Tag>(name)? {
            Some(tag) => tag.id(),
            None => db.insert(&Tag {
                name: name.to_string(),
                ..Tag::template()
            })?,
        };
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    Ok(ids)
}

/// `iter task edit --tag/--untag`: only the task's tags change, no editor.
fn task_retag(
    app: &App,
    task_ref: Option<&str>,
    tags: &[String],
    untags: &[String],
) -> Result<()> {
    let db = &app.db;
    let (project, task) = task_or_current(db, task_ref)?;
    let id = task.id();
    let added = ensure_tags(db, tags)?;
    let untags: Vec<&str> = untags.iter().map(|n| n.trim()).collect();
    let mut ids: Vec<i64> = db
        .tags_for_task(id)?
        .into_iter()
        .filter(|t| !untags.contains(&t.name.as_str()))
        .map(|t| t.id())
        .collect();
    for tag in added {
        if !ids.contains(&tag) {
            ids.push(tag);
        }
    }
    db.set_task_tags(id, &ids)?;
    println!("updated task '{}'", task.label(&project));
    Ok(())
}

/// Validates an edited task and resolves its tags before anything is written, so an unknown tag leaves the task untouched.
fn checked_tags(db: &Db, edit: &TaskEdit) -> Result<Vec<i64>> {
    require_name("task", &edit.task.name)?;
    db.ids_by_name::<Tag>(&edit.tags)
}

/// The project named in the edited buffer; empty or unchanged means the
/// one the task was opened with.
fn chosen_project(db: &Db, current: &Project, typed: &str) -> Result<Project> {
    match typed.trim() {
        "" => Ok(current.clone()),
        name if name == current.name => Ok(current.clone()),
        name => db.resolve::<Project>(name),
    }
}

fn task_edit(app: &App, task_ref: Option<&str>) -> Result<()> {
    let db = &app.db;
    let (project, existing) = task_or_current(db, task_ref)?;
    let id = existing.id();
    let tags = db.tags_for_task(id)?.into_iter().map(|t| t.name).collect();
    let template = TaskEdit {
        task: existing,
        tags,
        project: project.name.clone(),
    };
    edited(&template, "task", "updated", |mut edit| {
        let tag_ids = checked_tags(db, &edit)?;
        let target = chosen_project(db, &project, &edit.project)?;
        db.move_task(id, &mut edit.task, &project, &target)?;
        db.update(id, &edit.task)?;
        db.set_task_tags(id, &tag_ids)?;
        Ok(format!("updated task '{}'", edit.task.label(&target)))
    })
}

fn task_delete(app: &App, mux: &dyn Multiplexer, task_ref: Option<&str>) -> Result<()> {
    let db = &app.db;
    let (project, task) = task_or_current(db, task_ref)?;
    let task_id = task.id();
    let display = task.label(&project).to_string();
    let session_config = db.find_session_config_by_task(task_id)?;

    // delete the row before tearing down tmux/the worktree: from inside the task's own session `kill-session` takes this pane down too
    db.delete::<Task>(task_id)?;

    if let Some(session_config) = session_config {
        teardown_and_kill(db, mux, &project, &session_config);
    }
    println!("deleted task '{display}'");
    Ok(())
}

fn task_info(app: &App, task_ref: Option<&str>, opts: &ReportOpts) -> Result<()> {
    let db = &app.db;
    let (project, task) = task_or_current(db, task_ref)?;
    let now = app.now;
    let range = opts.range(now)?;
    let sessions = db.sessions_for_task_in_range(task.id(), range.from, range.to)?;

    let info = TaskInfo {
        header: report_header(
            (task.label(&project).to_string(), &task.description),
            &task,
            range,
            &sessions,
            now,
        )?,
        sessions: session_rows(&sessions, now, range.single_day().is_none()),
    };
    print_report(&info, opts.format)
}

/// Prints the label (`<project>/<title> #<id>`) of every task matching both filters, as a YAML
/// list; empty `statuses` and `None` project mean all.
pub(crate) fn task_list(
    app: &App,
    project_filter: Option<&str>,
    statuses: &[TaskStatus],
) -> Result<()> {
    let labels = app.db.task_labels(project_filter, statuses)?;
    print_yaml(&labels.iter().map(ToString::to_string).collect::<Vec<_>>())
}

fn task_done(app: &App, mux: &dyn Multiplexer, task_ref: Option<&str>, save: bool) -> Result<()> {
    let db = &app.db;
    let (project, mut task) = task_or_current(db, task_ref)?;
    let task_id = task.id();
    let display = task.label(&project).to_string();
    let session_config = db.find_session_config_by_task(task_id)?;

    // tearing the worktree down under an agent that is working in it would
    // take its commits with it
    if let Ok(dir) = iter_core::agent::taskdir::of(db, &project, &task)
        && let Some(run) = dir.runs().into_iter().find(|r| r.is_running())
    {
        return Err(IterError::CommandFailed(format!(
            "run {} of '{display}' is still working (pid {}): wait for it to finish, or stop it, and try again",
            run.n,
            run.pid.map_or("?".to_string(), |p| p.to_string())
        )));
    }

    // `--save` goes first: it can stop halfway, and a stopped merge must leave the task `wip` with its worktree, where the half-done merge waits
    if save {
        let session_config = session_config
            .as_ref()
            .ok_or_else(|| IterError::NothingToSave(display.clone()))?;
        save_to_default_branch(&project, session_config, &display)?;
    }

    close_open_session(db, task_id, None)?;

    // persist the status before tearing down the session-config: `kill-session` from inside the task's own session takes this pane too
    task.status = TaskStatus::Done;
    db.update(task_id, &task)?;

    // what the task's own notes said goes into today's note of the project,
    // and the wiki may be asked to learn from it; neither stops the task
    // from being done
    match ingest_job::bridge_task_notes(app, &project, &task) {
        Ok(0) => {}
        Ok(n) => println!(
            "added {n} notes of the task to today's note of '{}'",
            project.name
        ),
        Err(e) => eprintln!("warning: the task's notes were not added to today's note: {e}"),
    }
    ingest::auto(app, &project, &task, Trigger::TaskDone);

    if let Some(session_config) = session_config {
        teardown_and_kill(db, mux, &project, &session_config);
    }

    println!("task '{display}' marked done");
    Ok(())
}

/// What `--save` does: merges the default branch into the task's worktree first (conflicts happen there), then the task's branch into the default branch (a fast-forward).
/// A stopped merge is left as git left it (conflicted files and `MERGE_HEAD` are the report); nothing is torn down or marked done; commit and re-run `--save`.
fn save_to_default_branch(
    project: &Project,
    session_config: &SessionConfig,
    display: &str,
) -> Result<()> {
    require_github(project)?;
    require_git_repo(project)?;
    let (Some(worktree), Some(branch)) = (
        session_config.worktree_path.as_deref(),
        session_config.github_branch.as_deref(),
    ) else {
        return Err(IterError::NothingToSave(display.to_string()));
    };
    let default_branch = &project.default_branch;

    // the second merge lands in whatever `base_path` has checked out, so verify that branch before the first merge, not after a merge commit exists
    if git::current_branch(&project.base_path).as_deref() != Some(default_branch.as_str()) {
        return Err(IterError::NotOnDefaultBranch {
            path: project.base_path.clone(),
            branch: default_branch.clone(),
        });
    }

    if !git::merge(worktree, default_branch)? {
        return Err(IterError::MergeStopped {
            branch: default_branch.clone(),
            into: branch.to_string(),
            path: worktree.to_string(),
        });
    }
    if !git::merge(&project.base_path, branch)? {
        return Err(IterError::MergeStopped {
            branch: branch.to_string(),
            into: default_branch.clone(),
            path: project.base_path.clone(),
        });
    }
    println!("merged '{branch}' into '{default_branch}'");
    Ok(())
}

fn task_weekday(app: &App, task_ref: Option<&str>) -> Result<()> {
    let db = &app.db;
    let (project, task) = task_or_current(db, task_ref)?;
    let now = app.now;
    let sessions = db.sessions_for_task(task.id())?;
    let report = WeekdayReport {
        name: task.label(&project).to_string(),
        weekdays: weekday_averages(&sessions, now),
    };
    print_yaml(&report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iter_core::models::{Project, TaskStatus};

    fn mux() -> TmuxClient<'static> {
        let runner: &'static iter_core::process::fake::FakeRunner = Box::leak(Box::default());
        TmuxClient::new(runner, false)
    }

    fn app() -> App {
        App::with_db(iter_core::fixtures::db())
    }

    fn seed(app: &App) -> (i64, i64) {
        let project = Project {
            tmux: false,
            auto_branch: false,
            ..iter_core::fixtures::project("proj")
        };
        let project_id = app.db.insert(&project).expect("project inserts");
        let task = Task {
            status: TaskStatus::Wip,
            ..iter_core::fixtures::task(project_id, "a task")
        };
        let task_id = app.db.insert(&task).expect("task inserts");
        (project_id, task_id)
    }

    #[test]
    fn marking_a_task_done_closes_its_open_session_and_persists_the_status() {
        let app = app();
        let (_, task_id) = seed(&app);
        let start = iter_core::fixtures::dt("2026-09-01 09:00");
        iter_core::fixtures::add_session(&app.db, task_id, start, None);

        task_done(&app, &mux(), Some("proj/a task"), false).expect("task done succeeds");

        let task: Task = app.db.get(task_id).expect("reload").expect("task exists");
        assert_eq!(task.status, TaskStatus::Done);
        assert!(
            app.db
                .open_session_for_task(task_id)
                .expect("query")
                .is_none(),
            "the open session should have been closed"
        );
    }

    /// `--save` with no session-config merges nothing and leaves the task `wip`.
    #[test]
    fn saving_a_task_with_no_worktree_is_an_error_that_changes_nothing() {
        let app = app();
        let (_, task_id) = seed(&app);

        let error =
            task_done(&app, &mux(), Some("proj/a task"), true).expect_err("nothing to save");
        assert!(error.to_string().contains("proj/a task"), "{error}");

        let task: Task = app.db.get(task_id).expect("reload").expect("task exists");
        assert_eq!(task.status, TaskStatus::Wip);
    }

    /// R3 of task 302: with several runs working at once, `done` still refuses while any of them
    /// works, and leaves the task `wip`. The task folder is in a scratch notes folder.
    #[test]
    fn done_refuses_while_any_of_several_runs_works() {
        use iter_core::agent::{
            run::Harness,
            solve::{self, Observer, Spec},
        };
        struct Silent;
        impl Observer for Silent {}
        let app = app();
        let notes = iter_core::fixtures::scratch("cli-done-parallel");
        iter_core::fixtures::install_instructions(&notes);
        let project = Project {
            tmux: false,
            auto_branch: false,
            notes_path: notes.display().to_string(),
            ..iter_core::fixtures::project("proj")
        };
        let project_id = app.db.insert(&project).expect("project inserts");
        let task_id = app
            .db
            .insert(&Task {
                status: TaskStatus::Wip,
                ..iter_core::fixtures::task(project_id, "a task")
            })
            .expect("task inserts");
        let spec = Spec {
            agent: Harness::Claude,
            model: "haiku".into(),
            effort: "low".into(),
        };
        let name = "proj/a task";
        let (a, _) = solve::open_subagent(&app, name, "plan", &spec, &Silent).expect("run a");
        let (b, _) = solve::open_subagent(&app, name, "check", &spec, &Silent)
            .expect("run b opens beside run a");

        let refused = task_done(&app, &mux(), Some(name), false).expect_err("a and b work");
        assert!(refused.to_string().contains("still working"), "{refused}");
        solve::close(&app, name, a, iter_core::agent::run::Status::Done, "").expect("a closes");
        let refused = task_done(&app, &mux(), Some(name), false).expect_err("b still works");
        assert!(
            refused.to_string().contains(&format!("run {b} ")),
            "{refused}"
        );
        let task: Task = app.db.get(task_id).expect("reload").expect("task exists");
        assert_eq!(task.status, TaskStatus::Wip);
        let _ = std::fs::remove_dir_all(&notes);
    }

    #[test]
    fn an_unknown_task_ref_is_reported() {
        let app = app();
        seed(&app);
        let error = task_done(&app, &mux(), Some("proj/nope"), false).expect_err("no such task");
        assert!(error.to_string().contains("nope"), "{error}");
        let error =
            task_done(&app, &mux(), Some("nosuch/a task"), false).expect_err("no such project");
        assert!(error.to_string().contains("nosuch"), "{error}");
    }

    #[test]
    fn listing_narrows_by_project_and_status() {
        let app = app();
        seed(&app);
        let done = Task {
            status: TaskStatus::Done,
            ..iter_core::fixtures::task(1, "finished")
        };
        app.db.insert(&done).expect("task inserts");

        let unfinished =
            crate::complete::offered(&app.db, None, &[TaskStatus::Queue, TaskStatus::Wip])
                .expect("refs");
        assert_eq!(unfinished, [("proj/a task".to_string(), None)]);
        assert_eq!(
            crate::complete::offered(&app.db, Some("nosuch"), &[])
                .expect("refs")
                .len(),
            0
        );
        task_list(&app, Some("proj"), &[]).expect("task list succeeds");
    }
}
