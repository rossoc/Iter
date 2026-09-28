//! Addressing: turning what the user typed -- or the tmux session they are
//! sitting in -- into the project, task or organization it names.
//!
//! Every command starts here, which is why these live together rather than
//! with any one of them: `<project>/<task>` is parsed by `session`, `task`
//! and `comment` alike, and the "or the current tmux session" fallback is
//! shared by all three plus `project` and `organization`.

use crate::db::{Db, Table};
use crate::error::{IterError, Result};
use crate::models::{Project, ProjectGroup, Task, split_task_ref, task_ref};
use crate::{git, tmux};

/// How a task is named everywhere the CLI speaks about one: the exact
/// `<project>/<task>` syntax [`parse_task_ref`] reads back, written in the
/// one place its inverse lives so the two can't drift apart.
pub(crate) fn task_display(project: &Project, task: &Task) -> String {
    task_ref(&project.name, &task.name)
}

/// Splits `<project>/<task>` on the first `/`.
pub(crate) fn parse_task_ref(reference: &str) -> Result<(&str, &str)> {
    split_task_ref(reference).ok_or_else(|| IterError::InvalidTaskRef(reference.to_string()))
}

pub(crate) fn resolve_task(db: &Db, task_ref: &str) -> Result<(Project, Task)> {
    let (project_name, task_name) = parse_task_ref(task_ref)?;
    let project = db.resolve::<Project>(project_name)?;
    let task = find_task_or_err(db, &project, task_name)?;
    Ok((project, task))
}

/// The row `id` points at, or the error saying `what` the `of` named is gone.
fn follow<T: Table>(db: &Db, id: i64, what: &'static str, of: &'static str) -> Result<T> {
    db.get::<T>(id)?.ok_or(IterError::Orphan { what, of })
}

/// Resolves the project + task for the tmux session this process is
/// running inside -- the same lookup `iter t` prints.
pub(crate) fn current_session_task(db: &Db) -> Result<(Project, Task)> {
    let name = tmux::current_session_name().ok_or(IterError::NotInTmux)?;
    let session_config = db
        .find_session_config_by_tmux_name(&name)?
        .ok_or_else(|| IterError::UntrackedTmuxSession(name.clone()))?;
    let task: Task = follow(db, session_config.task_id, "task", "session")?;
    let project = follow(db, task.project_id, "project", "task")?;
    Ok((project, task))
}

/// Resolves an explicit `<project>/<task>` ref, or -- when none is given --
/// the task of the tmux session this process is running inside.
pub(crate) fn resolve_task_or_current(db: &Db, task_ref: Option<&str>) -> Result<(Project, Task)> {
    match task_ref {
        Some(r) => resolve_task(db, r),
        None => current_session_task(db),
    }
}

/// Resolves an explicit project name, or -- when none is given -- the
/// project of the tmux session this process is running inside.
pub(crate) fn resolve_project_or_current(db: &Db, name: Option<&str>) -> Result<Project> {
    match name {
        Some(n) => db.resolve(n),
        None => current_session_task(db).map(|(project, _)| project),
    }
}

/// Resolves an explicit board/organization name, or -- when none is given
/// -- the one the tmux session's project is in. Erroring when that project
/// is in none is deliberate: membership is optional by design, so there's
/// no sensible "current" one to fall back to.
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

/// One named task of `project`, or the same "no such task" error a
/// `<project>/<task>` ref would have raised.
pub(crate) fn find_task_or_err(db: &Db, project: &Project, name: &str) -> Result<Task> {
    db.find_task(project.id(), name)?
        .ok_or_else(|| IterError::TaskNotFound {
            project: project.name.clone(),
            task: name.to_string(),
        })
}

/// The project a `task pull`/`task push` runs against: resolved the usual
/// way, then held to the two things `gh` needs of it -- the project has to
/// be marked `github`, and `base_path` has to really be a repo, since that
/// is the only way `gh` learns which repo it's talking about.
pub(crate) fn github_project(db: &Db, name: Option<&str>) -> Result<Project> {
    let project = resolve_project_or_current(db, name)?;
    require_github(&project)?;
    require_git_repo(&project)?;
    Ok(project)
}

/// Holds `project` to having the github integration switched on. The check
/// a command makes when it is about to talk to `gh` at all.
pub(crate) fn require_github(project: &Project) -> Result<()> {
    match project.github {
        true => Ok(()),
        false => Err(IterError::GithubDisabled(project.name.clone())),
    }
}

/// Holds `project`'s `base_path` to really being a repo. Separate from
/// [`require_github`] because they fail for different reasons and not every
/// caller needs both: `gh` infers the repo from the directory's remote, so
/// a project marked github whose path is not a repo leaves `gh` guessing --
/// while `iter session new` needs the repo (for the worktree) whether or
/// not anything is going to talk to `gh`.
pub(crate) fn require_git_repo(project: &Project) -> Result<()> {
    match git::is_git_repo(&project.base_path) {
        true => Ok(()),
        false => Err(IterError::NotAGitRepo {
            name: project.name.clone(),
            path: project.base_path.clone(),
        }),
    }
}
