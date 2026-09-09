//! `iter task pull` / `iter task push` -- reconciling a project's tasks
//! with its GitHub issues.
//!
//! Split out of [`super::task`] because it is a subject of its own: every
//! `gh` call the CLI makes in bulk is here. What it does *not* contain is
//! the deciding -- [`crate::sync`] plans each issue and task as plain data,
//! and this module carries those plans out. That split is what lets the
//! decisions be tested without a `gh` in sight.

use crate::app::App;
use crate::db::Db;
use crate::db::Table;
use crate::error::{IterError, Result};
use crate::models::{Project, Task, TaskStatus};
use crate::utils::resolve::{find_task_or_err, github_project, task_display};
use crate::{git, github, sync};

/// Sets a task's issue link, status and description -- in the database, and
/// in the copy in hand. Both, because a pull plans each issue against the
/// tasks it's holding, so a row it has already written has to read back as
/// written. `status`/`description` are `None` when this issue says nothing
/// about them, which is what makes one write serve all three changes.
fn relink_task(
    db: &Db,
    task: &mut Task,
    issue: i64,
    status: Option<TaskStatus>,
    description: Option<&str>,
) -> Result<()> {
    task.github_issue = Some(issue);
    if let Some(status) = status {
        task.status = status;
    }
    if let Some(description) = description {
        task.description = description.to_string();
    }
    db.update(task.id(), task)
}

/// The issues a pull works from: every issue in the repo, or -- when
/// `--task` narrows it to one task -- just the issue that task tracks.
/// Scoped that way a pull can only ever update that one task, since every
/// other plan `plan_pull` makes needs an issue no task has claimed.
///
/// A `--task` that tracks no issue is an error rather than a no-op: there
/// is no other issue such a pull could have meant.
fn issues_to_pull(
    db: &Db,
    project: &Project,
    task_name: Option<&str>,
) -> Result<Vec<github::Issue>> {
    let Some(task_name) = task_name else {
        return github::list_issues(&project.base_path);
    };
    let task = find_task_or_err(db, project, task_name)?;
    let number = task
        .github_issue
        .ok_or_else(|| IterError::NoLinkedIssue(task_display(project, &task)))?;
    Ok(vec![github::fetch_issue(&project.base_path, number)?])
}

/// `iter task pull <project>` -- every issue in the project's repo brought
/// down: one nothing tracks becomes a task, and one already tracked hands
/// its open/closed state to the task's status (including reopened ->
/// `queue`, which is what lets `iter session new` pick the work up again).
/// `--task` narrows it to the one issue that task tracks, and `--body`
/// additionally overwrites descriptions with issue bodies.
///
/// Issues are the only input; no task is created, renamed or deleted for
/// want of one, so a task with no issue at all is simply not this command's
/// business -- `iter task push` is.
pub(crate) fn task_pull(
    app: &App,
    project_name: Option<&str>,
    task_name: Option<&str>,
    body: bool,
) -> Result<()> {
    let db = &app.db;
    let project = github_project(db, project_name)?;
    let project_id = project.id();
    let branch_prefix = git::branch_prefix(&project.branch_template);

    let issues = issues_to_pull(db, &project, task_name)?;
    // Read once, then kept current as the pull goes: each issue is planned
    // against what the ones before it did, so two issues sharing a title
    // read as the clash they are rather than colliding on the name index.
    let mut tasks = db.tasks_for_project(project_id)?;

    let (mut created, mut updated) = (0, 0);
    for issue in &issues {
        match sync::plan_pull(&tasks, issue, body) {
            sync::Pull::Create { status } => {
                let mut task = Task {
                    id: None,
                    project_id,
                    name: issue.title.clone(),
                    description: issue.body.clone(),
                    github_issue: Some(issue.number),
                    status,
                    branch_prefix: branch_prefix.clone(),
                };
                task.id = Some(db.insert(&task)?);
                println!(
                    "created task '{}' from issue #{} ({})",
                    task_display(&project, &task),
                    issue.number,
                    status.as_str()
                );
                tasks.push(task);
                created += 1;
            }
            sync::Pull::Update {
                task,
                link,
                status,
                description,
            } => {
                relink_task(
                    db,
                    &mut tasks[task],
                    issue.number,
                    status,
                    description.then_some(issue.body.as_str()),
                )?;
                println!(
                    "task '{}' <- issue #{}: {}",
                    task_display(&project, &tasks[task]),
                    issue.number,
                    pull_changes(link, status, description)
                );
                updated += 1;
            }
            sync::Pull::Unchanged => {}
            sync::Pull::Conflict { task } => eprintln!(
                "warning: issue #{} skipped -- task '{}' has the same name but tracks issue #{}",
                issue.number,
                task_display(&project, &tasks[task]),
                tasks[task]
                    .github_issue
                    .expect("a conflicting task is a linked one")
            ),
        }
    }

    println!(
        "pulled {} issue(s) from '{}': {created} created, {updated} updated",
        issues.len(),
        project.name
    );
    Ok(())
}

/// The tail of a pull's per-task line, naming what actually changed. One
/// pull can link a task, restatus it and rewrite its description all at
/// once, so the line says which of the three it did.
fn pull_changes(link: bool, status: Option<TaskStatus>, description: bool) -> String {
    let mut changes = Vec::new();
    if link {
        changes.push("linked".to_string());
    }
    if let Some(status) = status {
        changes.push(format!("now {}", status.as_str()));
    }
    if description {
        changes.push("description updated".to_string());
    }
    changes.join(", ")
}

/// What one task's push did to its issue -- what `task_push` counts.
enum Pushed {
    Opened,
    Edited,
    Nothing,
}

/// Pushes a single task to its GitHub issue: opens one, edits one, or
/// leaves it alone, per `sync::plan_push`.
///
/// A free function rather than a block inside `task_push`'s loop so that
/// one task's failure is a `Result` the caller can absorb, the way
/// `issues_to_pull` already splits the pull side up.
fn push_task(
    db: &Db,
    project: &Project,
    task: &mut Task,
    issues: &[github::Issue],
    display: &str,
    body: bool,
) -> Result<Pushed> {
    let plan = sync::plan_push(task, issues, body);
    // Read off the plan before it's taken apart: closing an issue is
    // what signs it, whether that's a new issue or an existing one.
    let signs = plan.closes();
    match plan {
        sync::Push::Create { state } => {
            let number = github::create_issue(
                &project.base_path,
                &task.name,
                &task.description,
                &project.github_project,
            )?;
            // Written back before the issue is closed below: if that
            // second call fails, the task still knows its issue, and
            // running `push` again finishes the job rather than opening
            // a second issue for the same task.
            relink_task(db, task, number, None, None)?;
            match project.github_project.is_empty() {
                true => println!("opened issue #{number} for '{display}'"),
                false => println!(
                    "opened issue #{number} for '{display}', on project '{}'",
                    project.github_project
                ),
            }
            if state == github::IssueState::Closed {
                sign_and_close(project, number, display, signs)?;
            }
            Ok(Pushed::Opened)
        }
        sync::Push::Update {
            number,
            state,
            body,
        } => {
            if body {
                github::set_issue_body(&project.base_path, number, &task.description)?;
                println!("issue #{number} <- '{display}': body updated");
            }
            match state {
                Some(github::IssueState::Closed) => {
                    sign_and_close(project, number, display, signs)?
                }
                Some(github::IssueState::Open) => {
                    github::set_issue_state(&project.base_path, number, github::IssueState::Open)?;
                    println!(
                        "reopened issue #{number} -- '{display}' is {}",
                        task.status.as_str()
                    );
                }
                None => {}
            }
            Ok(Pushed::Edited)
        }
        sync::Push::Unchanged => Ok(Pushed::Nothing),
        sync::Push::Missing { number } => {
            eprintln!("warning: '{display}' skipped -- issue #{number} isn't in this repo");
            Ok(Pushed::Nothing)
        }
    }
}

/// `iter task push <project>` -- every task sent up: one tracking no issue
/// gets one opened for it (and the number written back, so it's tracked
/// from then on), and one already tracking an issue has that issue closed
/// or reopened to match its status. `--task` narrows it to one task, and
/// `--body` additionally overwrites issue bodies with task descriptions.
///
/// Whenever a push closes an issue -- a `done` task's brand-new one, or one
/// going open -> closed -- the pusher is added to its assignees first,
/// signing it as done at least by them. `--add-assignee` adds, so whoever
/// is already assigned stays, and nothing is ever unassigned -- including
/// when an issue is reopened.
pub(crate) fn task_push(
    app: &App,
    project_name: Option<&str>,
    task_name: Option<&str>,
    body: bool,
) -> Result<()> {
    let db = &app.db;
    let project = github_project(db, project_name)?;
    let project_id = project.id();

    let mut tasks = match task_name {
        Some(name) => vec![find_task_or_err(db, &project, name)?],
        None => db.tasks_for_project(project_id)?,
    };

    // One listing, then a decision per task -- rather than asking `gh` for
    // the state of each task's issue one at a time.
    let issues = github::list_issues(&project.base_path)?;

    let (mut opened, mut edited, mut failed) = (0, 0, 0);
    for task in &mut tasks {
        let display = task_display(&project, task);
        // One task's `gh` calls are its own: an issue somebody locked, or a
        // single call that times out, skips that task and lets the rest of
        // the backlog through -- the warn-and-continue a `Missing` issue
        // already got. Ending the run there instead would also swallow the
        // summary below, leaving no way to tell how far the push got.
        match push_task(db, &project, task, &issues, &display, body) {
            Ok(Pushed::Opened) => opened += 1,
            Ok(Pushed::Edited) => edited += 1,
            Ok(Pushed::Nothing) => {}
            Err(e) => {
                eprintln!("warning: '{display}' didn't finish -- {e}");
                failed += 1;
            }
        }
    }

    let failures = match failed {
        0 => String::new(),
        failed => format!(", {failed} failed"),
    };
    println!(
        "pushed {} task(s) to '{}': {opened} issue(s) opened, {edited} edited{failures}",
        tasks.len(),
        project.name
    );
    Ok(())
}

/// Signs an issue as done by the pusher and closes it -- in that order, so
/// a run cut short leaves a signed open issue rather than a closed one
/// nobody is named on.
///
/// A failure to assign is reported but doesn't stop the close: assigning
/// needs write access to the repo, and losing the state sync -- the part
/// that keeps a task and its issue agreeing -- over a signature would be
/// the worse trade.
fn sign_and_close(project: &Project, number: i64, display: &str, sign: bool) -> Result<()> {
    if sign && let Err(e) = github::assign_self(&project.base_path, number) {
        eprintln!("warning: couldn't assign yourself to issue #{number}: {e}");
    }
    github::set_issue_state(&project.base_path, number, github::IssueState::Closed)?;
    println!("closed issue #{number} -- '{display}' is done");
    Ok(())
}
