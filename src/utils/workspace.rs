//! A task's workspace: the git worktree, branch and tmux session `iter session new` builds, and
//! their teardown.
//!
//! Kill-last invariant: `kill-session` takes down every pane, including this process when run from
//! inside that session, so nothing after it is guaranteed to run. Hence (1) write the database
//! before tearing down, and (2) kill last: [`teardown_session_config`] returns the tmux session to
//! kill instead of killing it, so `project delete` can kill after its loop; [`teardown_and_kill`]
//! is for callers where nothing follows.

use crate::db::{Db, Table};
use crate::error::Result;
use crate::models::{Project, SessionConfig, Task};
use crate::{git, tmux};

/// Makes the git worktree and the branch for `task` under the project's
/// `.iter-worktrees`: `(branch, worktree path)`. `session new` and
/// `task solve` both start from here.
pub(crate) fn create_task_worktree(
    project: &Project,
    task: &Task,
    branch_override: Option<&str>,
) -> Result<(String, String)> {
    // An empty prefix (blank, or a task from before the field) falls back to the project's template.
    let prefix = match task.branch_prefix.trim() {
        "" => git::branch_prefix(&project.branch_template),
        prefix => prefix.to_string(),
    };
    // One slug for branch and directory: names that slugify alike would collide on both, and
    // nothing renames the directory.
    let slug = git::task_slug(&task.name, task.id());
    let worktrees = std::path::Path::new(&project.base_path).join(".iter-worktrees");
    let slug = match worktrees.join(&slug).exists() {
        true => format!("{slug}-{}", task.id()),
        false => slug,
    };
    let branch = branch_override
        .map(|b| b.to_string())
        .unwrap_or_else(|| format!("{prefix}{slug}"));
    let path_str = worktrees.join(&slug).to_string_lossy().to_string();
    git::create_worktree(&project.base_path, &branch, &path_str)?;
    Ok((branch, path_str))
}

/// Removes the worktree, branch and session-config row, for `task done`/`task delete`/`project
/// delete`. Does not kill the tmux session; it returns it (see the module doc).
#[must_use = "the tmux session still needs killing"]
pub(crate) fn teardown_session_config<'a>(
    db: &Db,
    project: &Project,
    session_config: &'a SessionConfig,
) -> Option<&'a str> {
    remove_worktree_and_branch(
        project,
        session_config.worktree_path.as_deref(),
        session_config.github_branch.as_deref(),
    );
    if let Some(id) = session_config.id
        && let Err(e) = db.delete::<SessionConfig>(id)
    {
        eprintln!("warning: failed to delete session-config row: {e}");
    }
    session_config.tmux_session_name.as_deref()
}

/// The on-disk half of teardown, shared with `session_new`'s unwind (worktree and branch but no row
/// yet). Failures warn and carry on. The branch goes after the worktree, as `git::delete_branch`
/// requires.
pub(crate) fn remove_worktree_and_branch(
    project: &Project,
    worktree: Option<&str>,
    branch: Option<&str>,
) {
    if let Some(worktree) = worktree
        && let Err(e) = git::remove_worktree(&project.base_path, worktree)
    {
        eprintln!("warning: {e}");
    }
    if let Some(branch) = branch
        && let Err(e) = git::delete_branch(&project.base_path, branch)
    {
        eprintln!("warning: {e}");
    }
}

/// [`teardown_session_config`] for a lone session, killing at once; fine where nothing runs after.
pub(crate) fn teardown_and_kill(db: &Db, project: &Project, session_config: &SessionConfig) {
    if let Some(name) = teardown_session_config(db, project, session_config) {
        tmux::kill_session(name);
    }
}

/// Undoes `session_new` for a failure before the row exists (nothing could tear the worktree down
/// later). Kills the tmux session first: it is detached and unattached, so this cannot take the
/// running process with it.
pub(crate) fn unwind_session_setup(
    project: &Project,
    worktree: Option<&str>,
    branch: Option<&str>,
    tmux_session: Option<&str>,
) {
    if let Some(name) = tmux_session {
        tmux::kill_session(name);
    }
    remove_worktree_and_branch(project, worktree, branch);
}
