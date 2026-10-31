//! Addressing: turning what the user typed, or the tmux session they are in, into the project,
//! task or organization it names.

use crate::db::{Db, Table};
use crate::error::{IterError, Result};
use crate::models::{Project, ProjectGroup, Task, split_task_ref, task_ref};
use crate::{git, tmux};

/// A task as `<project>/<task>`, the syntax [`parse_task_ref`] reads back.
pub(crate) fn task_display(project: &Project, task: &Task) -> String {
    task_ref(&project.name, &task.name)
}

/// The task's issue number, or the "no linked issue" error.
pub(crate) fn linked_issue(project: &Project, task: &Task) -> Result<i64> {
    task.github_issue
        .ok_or_else(|| IterError::NoLinkedIssue(task_display(project, task)))
}

pub(crate) fn parse_task_ref(reference: &str) -> Result<(&str, &str)> {
    split_task_ref(reference).ok_or_else(|| IterError::InvalidTaskRef(reference.to_string()))
}

/// A task by `<project>/<task>`, or by its number (`12`: what a command run
/// by an agent says, with no space to quote).
pub(crate) fn resolve_task(db: &Db, task_ref: &str) -> Result<(Project, Task)> {
    if !task_ref.is_empty()
        && task_ref.bytes().all(|b| b.is_ascii_digit())
        && let Ok(id) = task_ref.parse::<i64>()
    {
        let task: Task = db.get(id)?.ok_or_else(|| IterError::NotFound {
            kind: "task",
            name: task_ref.to_string(),
        })?;
        let project = follow(db, task.project_id, "project", "task")?;
        return Ok((project, task));
    }
    let (project_name, task_name) = parse_task_ref(task_ref)?;
    let project = db.resolve::<Project>(project_name)?;
    let task = find_task_or_err(db, &project, task_name)?;
    Ok((project, task))
}

/// The row `id` points at, or the error saying `what` the `of` named is gone.
fn follow<T: Table>(db: &Db, id: i64, what: &'static str, of: &'static str) -> Result<T> {
    db.get::<T>(id)?.ok_or(IterError::Orphan { what, of })
}

/// The project + task of the tmux session this process runs in.
pub(crate) fn current_session_task(db: &Db) -> Result<(Project, Task)> {
    let name = tmux::current_session_name().ok_or(IterError::NotInTmux)?;
    let session_config = db
        .find_session_config_by_tmux_name(&name)?
        .ok_or_else(|| IterError::UntrackedTmuxSession(name.clone()))?;
    let task: Task = follow(db, session_config.task_id, "task", "session")?;
    let project = follow(db, task.project_id, "project", "task")?;
    Ok((project, task))
}

pub(crate) fn resolve_task_or_current(db: &Db, task_ref: Option<&str>) -> Result<(Project, Task)> {
    match task_ref {
        Some(r) => resolve_task(db, r),
        None => current_session_task(db),
    }
}

pub(crate) fn resolve_project_or_current(db: &Db, name: Option<&str>) -> Result<Project> {
    match name {
        Some(n) => db.resolve(n),
        None => current_session_task(db).map(|(project, _)| project),
    }
}

/// An explicit board/organization name, else the one the tmux session's project is in. Erroring
/// when it is in none is deliberate: membership is optional, so there is no sensible "current" one.
pub(crate) fn resolve_group_or_current<G: ProjectGroup>(db: &Db, name: Option<&str>) -> Result<G> {
    if let Some(name) = name {
        return db.resolve(name);
    }
    let project = current_session_task(db)?.0;
    let id = G::group_of(&project).ok_or_else(|| IterError::NotAMember {
        project: project.name.clone(),
        relation: G::NOT_A_MEMBER,
    })?;
    follow(db, id, G::KIND, "project")
}

pub(crate) fn find_task_or_err(db: &Db, project: &Project, name: &str) -> Result<Task> {
    db.find_task(project.id(), name)?
        .ok_or_else(|| IterError::TaskNotFound {
            project: project.name.clone(),
            task: name.to_string(),
        })
}

/// The project a `task pull`/`push` runs against: resolved as usual, then held to `github` and a
/// real repo, which is how `gh` learns the repo.
pub(crate) fn github_project(db: &Db, name: Option<&str>) -> Result<Project> {
    let project = resolve_project_or_current(db, name)?;
    require_github(&project)?;
    require_git_repo(&project)?;
    Ok(project)
}

pub(crate) fn require_github(project: &Project) -> Result<()> {
    match project.github {
        true => Ok(()),
        false => Err(IterError::GithubDisabled(project.name.clone())),
    }
}

/// Holds `project`'s `base_path` to being a repo. Separate from [`require_github`]: `gh` infers the
/// repo from the directory's remote, while `iter session new` needs it for the worktree whether or
/// not `gh` is involved.
pub(crate) fn require_git_repo(project: &Project) -> Result<()> {
    match git::is_git_repo(&project.base_path) {
        true => Ok(()),
        false => Err(IterError::NotAGitRepo {
            name: project.name.clone(),
            path: project.base_path.clone(),
        }),
    }
}
