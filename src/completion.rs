//! Shell completion callbacks, wired to the CLI by `ArgValueCompleter` in
//! [`crate::args`].
//!
//! They live in their own module rather than beside the commands because
//! `args` needs to name them: with them in the binary root, the CLI
//! *definition* had to import from the crate root that in turn imports it.
//!
//! Every one of them answers on a keystroke and has nowhere to report a
//! failure to, so each asks the database for exactly the strings it will
//! offer -- never a listing per project -- and treats any error as "nothing
//! to offer here".

use crate::db::{Db, Table, open_db};
use crate::error::Result;
use crate::models::{Named, Organization, Project, TaskStatus, task_ref};
use clap_complete::engine::CompletionCandidate;

/// The candidates a completer offers, out of the names `names` produces.
///
/// Every failure is silently no candidates: non-UTF-8 input, an unreadable
/// database and an empty table all mean the same thing to a shell waiting
/// on a TAB.
fn candidates(
    current: &std::ffi::OsStr,
    names: impl FnOnce(&Db) -> Result<Vec<String>>,
) -> Vec<CompletionCandidate> {
    let Some(current) = current.to_str() else {
        return Vec::new();
    };
    let Ok(names) = names(&open_db()) else {
        return Vec::new();
    };
    names
        .into_iter()
        .filter(|name| name.starts_with(current))
        .map(CompletionCandidate::new)
        .collect()
}

/// Every `T`'s name, in `T`'s listing order -- what both completion and
/// `iter <entity> list` are after, for any entity that has a name.
pub(crate) fn names<T: Table + Named>(db: &Db) -> Result<Vec<String>> {
    Ok(db
        .list::<T>()?
        .into_iter()
        .map(|item| item.name().to_string())
        .collect())
}

/// Dynamic completer for arguments naming a project (`project edit/delete/info`,
/// `task new --project`).
pub(crate) fn project_completer(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    candidates(current, names::<Project>)
}

/// Dynamic completer for arguments naming an organization.
pub(crate) fn organization_completer(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    candidates(current, names::<Organization>)
}

/// Dynamic completer for arguments naming a task as `<project>/<task>`,
/// offering only tasks at `status` when one is given.
fn task_completer_at(
    current: &std::ffi::OsStr,
    status: Option<TaskStatus>,
) -> Vec<CompletionCandidate> {
    candidates(current, |db| {
        Ok(db
            .task_refs(status)?
            .iter()
            .map(|(project, task)| task_ref(project, task))
            .collect())
    })
}

/// Dynamic completer for arguments naming any task.
pub(crate) fn task_completer(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    task_completer_at(current, None)
}

/// Dynamic completer for `task pull/push --task`, which takes a bare task
/// name rather than a `<project>/<task>` pair -- the project is already the
/// positional argument. Completion can't see that positional, so this
/// offers every task name there is and leaves narrowing to what you type.
pub(crate) fn task_name_completer(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    candidates(current, Db::task_names)
}

/// Dynamic completer for `session new`, which is only ever a sensible thing
/// to run on a task that hasn't been started: it refuses a task that already
/// has a session-config, and flips the one it does start to `wip`.
pub(crate) fn queued_task_completer(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    task_completer_at(current, Some(TaskStatus::Queue))
}
