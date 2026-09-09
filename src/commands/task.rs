//! `iter task` -- the CRUD and reporting half. The GitHub sync half is
//! [`super::sync`].

use crate::app::App;
use crate::args::{ReportOpts, TaskCommand};
use crate::commands::Run;
use crate::commands::sync::{task_pull, task_push};
use crate::config::config;
use crate::db::Table;
use crate::error::{IterError, Result};
use crate::md_edit::edited;
use crate::models::task_ref;
use crate::models::{Project, SessionConfig, Task, TaskStatus};
use crate::reporting::{
    Header, Report, TaskInfo, WeekdayReport, session_rows, settings_of, weekday_averages,
};
use crate::utils::clock::close_open_session;
use crate::utils::output::print_yaml;
use crate::utils::report::{resolve_range, total_minutes};
use crate::utils::resolve::{
    require_git_repo, require_github, resolve_project_or_current, resolve_task_or_current,
    task_display,
};
use crate::utils::workspace::teardown_and_kill;
use crate::{git, github};

/// Note that bare `iter task` is handled in `main` rather than here: it
/// means the unfinished work across every project, a filter no
/// `TaskCommand` variant expresses.
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
            Self::Done { task, save } => task_done(app, task.as_deref(), *save),
            Self::Pull {
                project,
                task,
                body,
            } => task_pull(app, project.as_deref(), task.as_deref(), *body),
            Self::Push {
                project,
                task,
                body,
            } => task_push(app, project.as_deref(), task.as_deref(), *body),
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
        let issue = github::fetch_issue(&project.base_path, issue_number)?;
        template.name = issue.title;
        template.description = issue.body;
        template.github_issue = Some(issue.number);
        // A task opened from an issue that's already closed starts `done`,
        // the same as one `iter task pull` brings down -- so the very next
        // `push` doesn't reopen the issue to match a status that was only
        // ever the template's default.
        template.status = github::status_for_issue_state(template.status, issue.state);
    }

    edited(&template, "task", "created", |mut task| {
        if task.name.trim().is_empty() {
            return Err(IterError::EmptyTaskName);
        }
        task.project_id = project_id; // never carried through the YAML
        db.insert(&task)?;
        Ok(format!("created task '{}'", task_display(&project, &task)))
    })
}

fn task_edit(app: &App, task_ref: Option<&str>) -> Result<()> {
    let db = &app.db;
    let (project, existing) = resolve_task_or_current(db, task_ref)?;
    let id = existing.id();
    let project_id = existing.project_id;
    edited(&existing, "task", "updated", |mut task| {
        task.project_id = project_id; // never carried through the YAML
        db.update(id, &task)?;
        Ok(format!("updated task '{}'", task_display(&project, &task)))
    })
}

fn task_delete(app: &App, task_ref: Option<&str>) -> Result<()> {
    let db = &app.db;
    let (project, task) = resolve_task_or_current(db, task_ref)?;
    let task_id = task.id();
    let display = task_display(&project, &task);
    let session_config = db.find_session_config_by_task(task_id)?;

    // Delete the task row (cascades to its session-config) before tearing
    // down tmux/the worktree: if we're running inside the task's own tmux
    // session, `kill-session` in the teardown takes down this pane too, so
    // anything after that point may never run.
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
        header: Header::new(
            task_display(&project, &task),
            &task.description,
            settings_of(&task)?,
            range,
            total_minutes,
        ),
        sessions: session_rows(&sessions, now, range.single_day().is_none()),
    };
    print!("{}", info.render(opts.format)?);
    Ok(())
}

/// Prints `<project>/<task>` for every task matching both filters, as a
/// YAML list. An empty `statuses` means every status, the same way `None`
/// for `project_filter` means every project.
pub(crate) fn task_list(
    app: &App,
    project_filter: Option<&str>,
    statuses: &[TaskStatus],
) -> Result<()> {
    let db = &app.db;
    let names: Vec<String> = db
        .task_refs(project_filter, statuses)?
        .iter()
        .map(|(project, task)| task_ref(project, task))
        .collect();
    print_yaml(&names)
}

fn task_done(app: &App, task_ref: Option<&str>, save: bool) -> Result<()> {
    let db = &app.db;
    let (project, mut task) = resolve_task_or_current(db, task_ref)?;
    let task_id = task.id();
    let display = task_display(&project, &task);
    let session_config = db.find_session_config_by_task(task_id)?;

    // `--save` goes first, before the clock, the status and the teardown:
    // it is the one step here that can stop halfway, and a merge that
    // stops has to leave the task exactly as it was -- still `wip`, with
    // its worktree still on disk, since that worktree is where the
    // half-done merge is waiting to be finished.
    if save {
        let session_config = session_config
            .as_ref()
            .ok_or_else(|| IterError::NothingToSave(display.clone()))?;
        save_to_default_branch(&project, session_config, &display)?;
    }

    close_open_session(db, task_id, None)?;

    // Persist the status change before tearing down the session-config: if
    // we're running inside the task's own tmux session, `kill-session` below
    // takes down every pane in it -- including this one -- so anything after
    // that point may never run.
    task.status = TaskStatus::Done;
    db.update(task_id, &task)?;

    if let Some(session_config) = session_config {
        teardown_and_kill(db, &project, &session_config);
    }

    println!("task '{display}' marked done");
    Ok(())
}

/// What `--save` does: lands the task's branch on the project's
/// `default_branch`, so the teardown that follows is throwing away a
/// worktree and a branch whose work is already home.
///
/// Two merges, in this order. The default branch goes into the task's
/// worktree first, so anything that conflicts conflicts *there* -- on the
/// task's own branch, in the checkout the work was done in -- and only then
/// does the task's branch go into the default branch, which by that point
/// is a fast-forward.
///
/// Either merge stopping is left exactly as git left it: the conflicted
/// files and the `MERGE_HEAD` beside them are the report, `git status` in
/// the worktree named by the error spells out what's outstanding, and
/// nothing here is torn down or marked done. Committing the merge and
/// re-running `--save` picks up where it stopped.
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

    // The second merge lands in whatever `base_path` has checked out, and
    // git will merge into a bystander branch as happily as into the right
    // one -- so this is checked before the first merge rather than
    // discovered after it, when there would already be a merge commit in
    // the worktree to explain.
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
    use crate::db::Db;
    use crate::models::{Project, Session, TaskStatus};
    use chrono::NaiveDate;

    /// A command driven against an in-memory database -- no config file, no
    /// `$XDG_CONFIG_HOME`, no editor. This is what `App::with_db` is for.
    fn app() -> App {
        App::with_db(Db::open(":memory:").expect("in-memory database opens"))
    }

    fn seed(app: &App) -> (i64, i64) {
        let project = Project {
            id: None,
            organization_id: None,
            name: "proj".to_string(),
            description: String::new(),
            base_path: "/tmp/proj".to_string(),
            github: false,
            tmux: false,
            auto_branch: false,
            branch_template: "feat/{task}".to_string(),
            default_branch: "main".to_string(),
            github_project: String::new(),
        };
        let project_id = app.db.insert(&project).expect("project inserts");
        let task = Task {
            id: None,
            project_id,
            name: "a task".to_string(),
            description: String::new(),
            github_issue: None,
            status: TaskStatus::Wip,
            branch_prefix: String::new(),
        };
        let task_id = app.db.insert(&task).expect("task inserts");
        (project_id, task_id)
    }

    /// `task done` has to close the clock *and* flip the status, and it has
    /// to persist both before any teardown -- a teardown run from inside
    /// the task's own tmux session can end the process partway.
    #[test]
    fn marking_a_task_done_closes_its_open_session_and_persists_the_status() {
        let app = app();
        let (_, task_id) = seed(&app);
        let open = Session {
            id: None,
            task_id,
            start: NaiveDate::from_ymd_opt(2026, 9, 1)
                .expect("valid date")
                .and_hms_opt(9, 0, 0)
                .expect("valid time"),
            end: None,
            message: None,
        };
        app.db.insert(&open).expect("session inserts");

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

    /// `--save` on a task that never had a session-config has nothing to
    /// merge, and the important half is what it *doesn't* do: the task is
    /// left `wip`, not quietly marked done on the strength of a merge that
    /// never happened.
    #[test]
    fn saving_a_task_with_no_worktree_is_an_error_that_changes_nothing() {
        let app = app();
        let (_, task_id) = seed(&app);

        let error = task_done(&app, Some("proj/a task"), true).expect_err("nothing to save");
        assert!(error.to_string().contains("proj/a task"), "{error}");

        let task: Task = app.db.get(task_id).expect("reload").expect("task exists");
        assert_eq!(task.status, TaskStatus::Wip);
    }

    /// A `<project>/<task>` that names nothing is an error rather than a
    /// silent no-op, and it names what was missing.
    #[test]
    fn an_unknown_task_ref_is_reported() {
        let app = app();
        seed(&app);
        let error = task_done(&app, Some("proj/nope"), false).expect_err("no such task");
        assert!(error.to_string().contains("nope"), "{error}");
        let error = task_done(&app, Some("nosuch/a task"), false).expect_err("no such project");
        assert!(error.to_string().contains("nosuch"), "{error}");
    }

    /// The bare `iter task` filter -- unfinished work only -- and the
    /// `--project` narrowing, both of which are now one SQL query.
    #[test]
    fn listing_narrows_by_project_and_status() {
        let app = app();
        seed(&app);
        let done = Task {
            id: None,
            project_id: 1,
            name: "finished".to_string(),
            description: String::new(),
            github_issue: None,
            status: TaskStatus::Done,
            branch_prefix: String::new(),
        };
        app.db.insert(&done).expect("task inserts");

        let unfinished = app
            .db
            .task_refs(None, &[TaskStatus::Queue, TaskStatus::Wip])
            .expect("refs");
        assert_eq!(unfinished, vec![("proj".to_string(), "a task".to_string())]);
        assert_eq!(
            app.db.task_refs(Some("nosuch"), &[]).expect("refs").len(),
            0
        );
        // ...and the command itself runs against the same database.
        task_list(&app, Some("proj"), &[]).expect("task list succeeds");
    }
}
