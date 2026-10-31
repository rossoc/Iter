//! `iter task` -- the CRUD and reporting half. The GitHub sync half is
//! [`super::sync`].

use crate::app::App;
use crate::args::{ReportOpts, TaskCommand};
use crate::commands::ingest;
use crate::wiki::ingest_job::{self, Trigger};
use crate::commands::sync::{task_pull, task_push};
use crate::commands::{Run, agent, solve};
use crate::config::config;
use crate::db::{Db, Table};
use crate::error::{IterError, Result};
use crate::git;
use crate::github::{self, GhCli, IssueTracker};
use crate::md_edit::edited;
use crate::models::require_name;
use crate::models::{Project, SessionConfig, Tag, Task, TaskEdit, TaskStatus};
use crate::reporting::{
    Header, TaskInfo, WeekdayReport, print_report, session_rows, weekday_averages,
};
use crate::utils::clock::close_open_session;
use crate::utils::output::print_yaml;
use crate::utils::report::{resolve_range, total_minutes};
use crate::utils::resolve::{
    require_git_repo, require_github, resolve_project_or_current, resolve_task_or_current,
    task_display,
};
use crate::utils::workspace::teardown_and_kill;

/// Bare `iter task` (unfinished work across projects) is handled in `main`: no `TaskCommand` variant expresses it.
impl Run for TaskCommand {
    fn run(&self, app: &App) -> Result<()> {
        match self {
            Self::New { project, issue } => task_new(app, project.as_deref(), *issue),
            Self::Edit { task } => task_edit(app, task.as_deref()),
            Self::Delete { task } => task_delete(app, task.as_deref()),
            Self::Info { task, report } => task_info(app, task.as_deref(), report),
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
            Self::Plan {
                task,
                show,
                stdin,
                mark,
                format,
            } => agent::plan(
                app,
                &agent::PlanRequest {
                    task: task.as_deref(),
                    show: *show,
                    stdin: *stdin,
                    mark,
                    format: *format,
                },
            ),
            Self::Prompt {
                task,
                kind,
                template,
                no_onboarding,
            } => agent::prompt_cmd(
                app,
                task.as_deref(),
                *kind,
                template.as_deref(),
                !*no_onboarding,
            ),
            Self::Solve {
                task,
                kind,
                agent,
                model,
                effort,
                yes,
                wait,
                extra,
            } => solve::solve(
                app,
                &solve::SolveRequest {
                    task: task.as_deref(),
                    kind: *kind,
                    agent: *agent,
                    model: model.as_deref(),
                    effort: effort.as_deref(),
                    yes: *yes,
                    wait: *wait,
                    extra,
                },
            ),
            Self::Runs { task, log } => solve::runs(app, task.as_deref(), *log),
            Self::RunWorker {
                task_id,
                run,
                extra,
            } => crate::agent::solve::worker(app, *task_id, *run, extra),
            Self::Done { task, save } => task_done(app, task.as_deref(), *save),
            Self::Pull {
                project,
                task,
                body,
            } => task_pull(
                app,
                &GhCli::system(),
                project.as_deref(),
                task.as_deref(),
                *body,
            ),
            Self::Push {
                project,
                task,
                body,
            } => task_push(
                app,
                &GhCli::system(),
                project.as_deref(),
                task.as_deref(),
                *body,
            ),
            Self::Weekday { task } => task_weekday(app, task.as_deref()),
        }
    }
}

fn task_new(app: &App, project_name: Option<&str>, issue: Option<i64>) -> Result<()> {
    let db = &app.db;
    let project = resolve_project_or_current(db, project_name)?;
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
        Ok(format!(
            "created task '{}'",
            task_display(&project, &edit.task)
        ))
    })
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
    let (project, existing) = resolve_task_or_current(db, task_ref)?;
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
        if target.id() != project.id() && db.find_session_config_by_task(id)?.is_some() {
            return Err(IterError::TaskHasSession(task_display(
                &project, &edit.task,
            )));
        }
        edit.task.project_id = target.id();
        db.update(id, &edit.task)?;
        db.set_task_tags(id, &tag_ids)?;
        Ok(format!(
            "updated task '{}'",
            task_display(&target, &edit.task)
        ))
    })
}

fn task_delete(app: &App, task_ref: Option<&str>) -> Result<()> {
    let db = &app.db;
    let (project, task) = resolve_task_or_current(db, task_ref)?;
    let task_id = task.id();
    let display = task_display(&project, &task);
    let session_config = db.find_session_config_by_task(task_id)?;

    // delete the row before tearing down tmux/the worktree: from inside the task's own session `kill-session` takes this pane down too
    db.delete::<Task>(task_id)?;

    if let Some(session_config) = session_config {
        teardown_and_kill(db, &project, &session_config);
    }
    println!("deleted task '{display}'");
    Ok(())
}

fn task_info(app: &App, task_ref: Option<&str>, opts: &ReportOpts) -> Result<()> {
    let db = &app.db;
    let (project, task) = resolve_task_or_current(db, task_ref)?;
    let now = app.now;
    let range = resolve_range(opts, now)?;
    let sessions = db.sessions_for_task_in_range(task.id(), range.from, range.to)?;
    let total_minutes = total_minutes(&sessions, now);

    let info = TaskInfo {
        header: Header::of(
            task_display(&project, &task),
            &task.description,
            &task,
            range,
            total_minutes,
        )?,
        sessions: session_rows(&sessions, now, range.single_day().is_none()),
    };
    print_report(&info, opts.format)
}

/// Prints `<project>/<task>` for every task matching both filters, as a YAML list; empty `statuses` and `None` project mean all.
pub(crate) fn task_list(
    app: &App,
    project_filter: Option<&str>,
    statuses: &[TaskStatus],
) -> Result<()> {
    print_yaml(&app.db.task_refs(project_filter, statuses)?)
}

fn task_done(app: &App, task_ref: Option<&str>, save: bool) -> Result<()> {
    let db = &app.db;
    let (project, mut task) = resolve_task_or_current(db, task_ref)?;
    let task_id = task.id();
    let display = task_display(&project, &task);
    let session_config = db.find_session_config_by_task(task_id)?;

    // tearing the worktree down under an agent that is working in it would
    // take its commits with it
    if let Ok(dir) = crate::agent::taskdir::of(db, &project, &task)
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
        teardown_and_kill(db, &project, &session_config);
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
    let (project, task) = resolve_task_or_current(db, task_ref)?;
    let now = app.now;
    let sessions = db.sessions_for_task(task.id())?;
    let report = WeekdayReport {
        name: task_display(&project, &task),
        weekdays: weekday_averages(&sessions, now),
    };
    print_yaml(&report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Project, TaskStatus};

    fn app() -> App {
        App::with_db(crate::fixtures::db())
    }

    fn seed(app: &App) -> (i64, i64) {
        let project = Project {
            tmux: false,
            auto_branch: false,
            ..crate::fixtures::project("proj")
        };
        let project_id = app.db.insert(&project).expect("project inserts");
        let task = Task {
            status: TaskStatus::Wip,
            ..crate::fixtures::task(project_id, "a task")
        };
        let task_id = app.db.insert(&task).expect("task inserts");
        (project_id, task_id)
    }

    #[test]
    fn marking_a_task_done_closes_its_open_session_and_persists_the_status() {
        let app = app();
        let (_, task_id) = seed(&app);
        let start = crate::fixtures::dt("2026-09-01 09:00");
        crate::fixtures::add_session(&app.db, task_id, start, None);

        task_done(&app, Some("proj/a task"), false).expect("task done succeeds");

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

        let error = task_done(&app, Some("proj/a task"), true).expect_err("nothing to save");
        assert!(error.to_string().contains("proj/a task"), "{error}");

        let task: Task = app.db.get(task_id).expect("reload").expect("task exists");
        assert_eq!(task.status, TaskStatus::Wip);
    }

    #[test]
    fn an_unknown_task_ref_is_reported() {
        let app = app();
        seed(&app);
        let error = task_done(&app, Some("proj/nope"), false).expect_err("no such task");
        assert!(error.to_string().contains("nope"), "{error}");
        let error = task_done(&app, Some("nosuch/a task"), false).expect_err("no such project");
        assert!(error.to_string().contains("nosuch"), "{error}");
    }

    #[test]
    fn listing_narrows_by_project_and_status() {
        let app = app();
        seed(&app);
        let done = Task {
            status: TaskStatus::Done,
            ..crate::fixtures::task(1, "finished")
        };
        app.db.insert(&done).expect("task inserts");

        let unfinished = app
            .db
            .task_refs(None, &[TaskStatus::Queue, TaskStatus::Wip])
            .expect("refs");
        assert_eq!(unfinished, vec!["proj/a task"]);
        assert_eq!(
            app.db.task_refs(Some("nosuch"), &[]).expect("refs").len(),
            0
        );
        task_list(&app, Some("proj"), &[]).expect("task list succeeds");
    }
}
