//! Shell completion callbacks, wired by `ArgValueCompleter` in [`crate::args`]. They answer on a
//! keystroke with nowhere to report failure, so each queries exactly the strings it offers and treats
//! any error as no candidates. Kept out of the binary root so `args` can name them without a cycle.

use crate::db::{Db, open_db};
use crate::error::Result;
use crate::models::{Board, Organization, Project, Tag, TaskStatus};
use clap_complete::engine::CompletionCandidate;

/// The candidates out of `names`. Every failure (non-UTF-8 input, unreadable database, empty
/// table) is no candidates.
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

pub(crate) fn project_completer(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    candidates(current, Db::names::<Project>)
}

pub(crate) fn board_completer(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    candidates(current, Db::names::<Board>)
}

pub(crate) fn tag_completer(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    candidates(current, Db::names::<Tag>)
}

pub(crate) fn organization_completer(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    candidates(current, Db::names::<Organization>)
}

/// Dynamic completer for arguments naming a task as `<project>/<task>`,
/// offering only tasks at one of `statuses` -- or every task, when it's
/// empty.
fn task_completer_at(
    current: &std::ffi::OsStr,
    statuses: &'static [TaskStatus],
) -> Vec<CompletionCandidate> {
    candidates(current, |db| db.task_refs(None, statuses))
}

pub(crate) fn task_completer(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    task_completer_at(current, &[])
}

/// For `task pull/push --task`, a bare name: completion can't see the project positional, so it
/// offers every name.
pub(crate) fn task_name_completer(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    candidates(current, Db::task_names)
}

/// Dynamic completer for `session new`, which is only ever a sensible thing
/// to run on a task that hasn't been started: it refuses a task that already
/// has a session-config, and flips the one it does start to `wip`.
pub(crate) fn queued_task_completer(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    task_completer_at(current, &[TaskStatus::Queue])
}
