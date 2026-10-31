//! What can go wrong in the agent workflow, as a type of its own.
//!
//! Every message here is the one the CLI and the pages have always shown;
//! the point of the type is that a caller can now tell a busy run from a
//! missing one without reading the text. It converts into [`IterError`] with
//! `?` (see `IterError::Agent`), which prints it unchanged.

use crate::agent::plan::MarkError;
use std::path::PathBuf;
use thiserror::Error;

pub type AgentResult<T> = std::result::Result<T, AgentError>;

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    // ---- runs
    #[error("no such run")]
    RunNotFound(u32),
    #[error("the run vanished")]
    RunVanished(u32),
    /// Another run of the same task is still working.
    #[error("run {run} is still working on '{task}': wait for it, or stop it first")]
    RunBusy { run: u32, task: String },
    #[error("run {run} is {status}, not queued")]
    RunNotQueued { run: u32, status: String },
    #[error("run {run} is {status}: nothing to stop")]
    RunNotStoppable { run: u32, status: String },
    #[error("run {0} has begun: it cannot be edited")]
    RunBegun(u32),
    #[error("run {0} has no log")]
    RunLogMissing(u32),
    #[error("no such task")]
    TaskGone,
    #[error("no such project")]
    ProjectGone,
    #[error("{0} is not a git repository")]
    NotAGitRepo(String),

    // ---- the buffer `solve` opens
    #[error("the buffer lost its front matter (the lines between `---`)")]
    BufferLostFront,
    #[error("front matter: {0}")]
    BufferFront(serde_yaml::Error),

    // ---- plans
    #[error("the plan is empty: nothing was saved")]
    EmptyPlan,
    #[error("there is no plan yet: write one with `iter task plan`")]
    NoPlan,
    #[error("{0}")]
    NoSuchStep(#[from] MarkError),
    #[error("`{0}` is not a state: todo, doing or done")]
    UnknownState(String),

    // ---- prompts and instructions
    #[error("cannot read {}: {source}", .path.display())]
    TemplateUnreadable {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("unknown placeholder {{{{{key}}}}} (known: {known})")]
    UnknownPlaceholder { key: String, known: String },
    #[error("front matter: {0}")]
    FrontMatterUnreadable(serde_yaml::Error),
    #[error("front matter: not a list of `key: value` lines")]
    FrontMatterNotMapping,
    #[error("front matter: `{0}` must be text")]
    FrontMatterNotText(String),
    #[error("front matter: `kind` is plan, develop, test, check or fix, not '{0}'")]
    FrontMatterKind(String),
    #[error("'{0}' is not a name: lowercase letters, digits and dashes")]
    InvalidName(String),
    #[error("'{0}' is built in and keeps its name")]
    BuiltInKeepsName(String),
    #[error("'{0}' is the name of a built-in instruction")]
    BuiltInName(String),
    #[error("an instruction needs a prompt under its front matter")]
    NoPrompt,
    #[error("kind is plan, develop, test, check or fix, not '{0}'")]
    FieldKind(String),
    #[error("permission: {0}")]
    FieldPermission(serde_yaml::Error),
    #[error("{0}")]
    Yaml(serde_yaml::Error),

    // ---- the wiki
    /// An agent holds the wiki's lock.
    #[error("an agent is already writing in this wiki")]
    WikiBusy,
    /// Its changes are being settled while an agent is still writing.
    #[error("an agent is writing in the wiki: wait for it to finish")]
    WikiWriting,
    #[error("an ingest is already running")]
    IngestRunning,
    #[error("could not take the wiki's lock")]
    WikiLockUnavailable,
    #[error("the agent failed: see {}", .0.display())]
    AgentFailed(PathBuf),
    #[error(
        "{} is not in a git repository, so the changes of an agent could not be reviewed: `git init` there first",
        .0.display()
    )]
    NotesNotInGit(PathBuf),
    #[error("the name is a file name: no slash, not empty")]
    ConceptName,
    #[error("no notes folder for the project")]
    NoNotesFolder,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::IterError;

    /// The messages are part of the interface: the CLI prints them and the
    /// pages show them.
    #[test]
    fn messages_read_as_they_always_have() {
        let busy = AgentError::RunBusy {
            run: 3,
            task: "web/login".into(),
        };
        assert_eq!(
            busy.to_string(),
            "run 3 is still working on 'web/login': wait for it, or stop it first"
        );
        assert_eq!(AgentError::RunNotFound(9).to_string(), "no such run");
        assert_eq!(
            AgentError::WikiBusy.to_string(),
            "an agent is already writing in this wiki"
        );
    }

    #[test]
    fn it_converts_into_iter_error_unchanged_and_keeps_its_type() {
        let e: IterError = AgentError::RunBegun(2).into();
        assert_eq!(e.to_string(), "run 2 has begun: it cannot be edited");
        assert!(matches!(e, IterError::Agent(AgentError::RunBegun(2))));
        let io: IterError = AgentError::Io(std::io::Error::other("disk")).into();
        assert_eq!(io.to_string(), "I/O error: disk");
    }
}
