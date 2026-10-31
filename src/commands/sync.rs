//! `iter task pull` / `iter task push`: carries out the plans of [`crate::sync`] against GitHub issues; every bulk `gh` call is here.

use std::collections::HashMap;

use crate::app::App;
use crate::db::Db;
use crate::db::Table;
use crate::error::Result;
use crate::github::{self, IssueTracker};
use crate::models::{Project, Task, TaskStatus};
use crate::utils::resolve::{find_task_or_err, github_project, linked_issue, task_display};
use crate::{git, sync};

/// Sets a task's issue link, status and description in the database and in the copy in hand (a pull plans each issue against the tasks it holds); `None` means the issue says nothing about it.
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

/// The issues a pull works from: all in the repo, or with `--task` just the one issue that task tracks (an error if it tracks none).
fn issues_to_pull(
    gh: &dyn IssueTracker,
    db: &Db,
    project: &Project,
    task_name: Option<&str>,
) -> Result<Vec<github::Issue>> {
    let Some(task_name) = task_name else {
        return gh.list(&project.base_path);
    };
    let task = find_task_or_err(db, project, task_name)?;
    let number = linked_issue(project, &task)?;
    Ok(vec![gh.fetch(&project.base_path, number)?])
}

/// `iter task pull <project>`: every issue comes down; an untracked one becomes a task, a tracked one hands its open/closed state to the task's status (reopened -> `queue`).
/// `--task` narrows it to one issue, `--body` also overwrites descriptions. No task is created, renamed or deleted for want of an issue.
pub(crate) fn task_pull(
    app: &App,
    gh: &dyn IssueTracker,
    project_name: Option<&str>,
    task_name: Option<&str>,
    body: bool,
) -> Result<()> {
    let db = &app.db;
    let project = github_project(db, project_name)?;
    let project_id = project.id();
    let branch_prefix = git::branch_prefix(&project.branch_template);

    let issues = issues_to_pull(gh, db, &project, task_name)?;
    // read once, then kept current: each issue is planned against what the ones before did, so two issues sharing a title read as a clash
    let mut tasks = sync::Tasks::new(db.tasks_for_project(project_id)?);

    let (mut created, mut updated) = (0, 0);
    db.batched(|| {
        for issue in &issues {
            match sync::plan_pull(&tasks, issue, body) {
                sync::Pull::Create { status } => {
                    let mut task = Task {
                        name: issue.title.clone(),
                        description: issue.body.clone(),
                        github_issue: Some(issue.number),
                        ..Task::template(project_id, branch_prefix.clone(), status)
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
                        tasks.link(task, issue.number),
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
        Ok(())
    })?;

    println!(
        "pulled {} issue(s) from '{}': {created} created, {updated} updated",
        issues.len(),
        project.name
    );
    Ok(())
}

/// The tail of a pull's per-task line: which of link, status and description changed.
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

enum Pushed {
    Opened,
    Edited,
    Nothing,
}

/// Pushes one task to its GitHub issue (opens, edits or leaves alone, per `sync::plan_push`); a free function so one task's failure is a `Result` the caller can absorb.
fn push_task(
    gh: &dyn IssueTracker,
    db: &Db,
    project: &Project,
    task: &mut Task,
    issues: &HashMap<i64, github::Issue>,
    display: &str,
    body: bool,
) -> Result<Pushed> {
    match sync::plan_push(task, issues, body) {
        sync::PushAction::Create { state } => {
            let number = gh.create(
                &project.base_path,
                &task.name,
                &task.description,
                &project.github_project,
            )?;
            // written back before the issue is closed: if the close fails the task still knows its issue, and running `push` again finishes instead of opening a second
            relink_task(db, task, number, None, None)?;
            match project.github_project.is_empty() {
                true => println!("opened issue #{number} for '{display}'"),
                false => println!(
                    "opened issue #{number} for '{display}', on project '{}'",
                    project.github_project
                ),
            }
            if state == github::IssueState::Closed {
                sign_and_close(gh, project, number, display)?;
            }
            Ok(Pushed::Opened)
        }
        sync::PushAction::Update {
            number,
            state,
            body,
        } => {
            if body {
                gh.set_body(&project.base_path, number, &task.description)?;
                println!("issue #{number} <- '{display}': body updated");
            }
            match state {
                Some(github::IssueState::Closed) => sign_and_close(gh, project, number, display)?,
                Some(github::IssueState::Open) => {
                    gh.set_state(&project.base_path, number, github::IssueState::Open)?;
                    println!(
                        "reopened issue #{number} -- '{display}' is {}",
                        task.status.as_str()
                    );
                }
                None => {}
            }
            Ok(Pushed::Edited)
        }
        sync::PushAction::Unchanged => Ok(Pushed::Nothing),
        sync::PushAction::Missing { number } => {
            eprintln!("warning: '{display}' skipped -- issue #{number} isn't in this repo");
            Ok(Pushed::Nothing)
        }
    }
}

/// `iter task push <project>`: a task tracking no issue gets one opened (number written back), a tracked one has its issue closed or reopened to match.
/// `--task` narrows to one task, `--body` also overwrites issue bodies. When a push closes an issue the pusher is first added to its assignees; `--add-assignee` adds, nothing is ever unassigned.
pub(crate) fn task_push(
    app: &App,
    gh: &dyn IssueTracker,
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

    let issues = issues_to_push(gh, &project, task_name.is_some(), &tasks)?;

    let (mut opened, mut edited, mut failed) = (0, 0, 0);
    for task in &mut tasks {
        let display = task_display(&project, task);
        // one task's `gh` calls are its own: a locked issue or a timeout skips that task (warn and continue) rather than ending the run and swallowing the summary
        match push_task(gh, db, &project, task, &issues, &display, body) {
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

/// The issues a push plans against, by number: one call for a whole project, just the one issue with `--task`.
fn issues_to_push(
    gh: &dyn IssueTracker,
    project: &Project,
    single: bool,
    tasks: &[Task],
) -> Result<HashMap<i64, github::Issue>> {
    let issues = match single {
        false => gh.list(&project.base_path)?,
        // a failed fetch (issue deleted or transferred) is warned about and left out, giving the same warn-and-skip `Missing` plan
        true => tasks
            .iter()
            .filter_map(|t| t.github_issue)
            .filter_map(|number| match gh.fetch(&project.base_path, number) {
                Ok(issue) => Some(issue),
                Err(e) => {
                    eprintln!("warning: couldn't fetch issue #{number}: {e}");
                    None
                }
            })
            .collect(),
    };
    Ok(issues.into_iter().map(|i| (i.number, i)).collect())
}

/// Signs an issue as done by the pusher, then closes it, so a run cut short leaves a signed open issue.
/// A failure to assign is reported but does not stop the close: assigning needs write access, and the state sync matters more.
fn sign_and_close(
    gh: &dyn IssueTracker,
    project: &Project,
    number: i64,
    display: &str,
) -> Result<()> {
    if let Err(e) = gh.assign_self(&project.base_path, number) {
        eprintln!("warning: couldn't assign yourself to issue #{number}: {e}");
    }
    gh.set_state(&project.base_path, number, github::IssueState::Closed)?;
    println!("closed issue #{number} -- '{display}' is done");
    Ok(())
}
