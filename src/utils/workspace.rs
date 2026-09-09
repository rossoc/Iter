//! A task's workspace: the git worktree, the branch and the tmux session
//! `iter session new` builds around it, and taking them back down again.
//!
//! # The kill-last invariant
//!
//! Killing a tmux session takes down every pane in it. Run `iter task done`
//! (or `task delete`, or `project delete`) from *inside* the very session
//! being torn down and `kill-session` takes this process with it -- so
//! nothing sequenced after that call is guaranteed to run.
//!
//! Two rules follow, and every caller here depends on them:
//!
//! 1. **Write the database first.** Delete the row, or persist the status
//!    change, *before* tearing the workspace down. A teardown that never
//!    returns must not leave the database saying the work is still open.
//! 2. **Kill last, and kill outside the loop.** [`teardown_session_config`]
//!    deliberately does not kill: it *returns* the tmux session that still
//!    needs killing, so a caller tearing down several (`project delete`)
//!    can kill them after its loop rather than have the first one end the
//!    process halfway through. [`teardown_and_kill`] is the convenience for
//!    the callers where nothing follows.

use crate::db::Db;
use crate::models::{Project, SessionConfig};
use crate::{git, tmux};

/// Tears down a task's session-config: removes its worktree (if any),
/// deletes its branch (if any), deletes the session-config row, and finally
/// kills its tmux session (if any). Called from `task done`, `task delete`
/// and `project delete`.
///
/// The tmux kill comes last, and deliberately isn't the thing anything else
/// here depends on: if this is run from inside that same tmux session,
/// `kill-session` takes down every pane in it -- including this one -- so
/// nothing after that call is guaranteed to run. Which is why the kill
/// isn't done here at all: the tmux session that still needs killing is
/// *returned*, so a caller tearing down several of them can do the killing
/// after its loop rather than have the first one end the process halfway
/// through.
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

/// The on-disk half of a session-config's teardown, shared with
/// `session_new`'s unwind path -- which has the same worktree and branch to
/// undo but no row to delete yet. Both halves warn and carry on: a failure
/// here shouldn't stop the rest of the cleanup.
///
/// The branch goes after the worktree, which `git::delete_branch` requires.
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

/// `teardown_session_config` for a lone session, killing its tmux session
/// straight away. Safe where nothing needs to run afterwards -- which is
/// every caller but `project_delete`, whose loop has to outlive the kill.
pub(crate) fn teardown_and_kill(db: &Db, project: &Project, session_config: &SessionConfig) {
    if let Some(name) = teardown_session_config(db, project, session_config) {
        tmux::kill_session(name);
    }
}

/// Undoes what `session_new` built, for a failure before the session-config
/// row exists. Without a row there is nothing to tear the worktree down
/// from later, so it has to happen here or not at all.
///
/// Killing the tmux session first: it was created detached and nothing has
/// attached to it yet, so this can't take the running process with it the
/// way `teardown_session_config` can.
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
