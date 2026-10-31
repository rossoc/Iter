use thiserror::Error;

/// The one error type for the app. External-tool failures are `Spawn` (couldn't run it, carrying
/// the `io::Error`) or `CommandFailed` (ran and failed; a message built at the call site).
#[derive(Debug, Error)]
pub enum IterError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("cannot locate a config directory -- set $XDG_CONFIG_HOME or $HOME")]
    NoConfigDir,

    #[error("failed to read config at {path}: {source}")]
    ConfigUnreadable {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid config at {path}: {source}")]
    InvalidConfig {
        path: String,
        #[source]
        source: serde_yaml::Error,
    },

    #[error(
        "couldn't read the edited buffer at {path}: {source}\n\
         your edits are still there -- fix the front matter and re-run"
    )]
    InvalidBuffer {
        path: String,
        #[source]
        source: serde_yaml::Error,
    },

    #[cfg(any(feature = "web", test))]
    #[error("the note would not read as a note: {0}")]
    InvalidNote(String),

    #[error("failed to run `{tool}`: {source}")]
    Spawn {
        tool: &'static str,
        #[source]
        source: std::io::Error,
    },

    #[error("{0}")]
    CommandFailed(String),

    /// The agent workflow's own errors.
    #[error("{0}")]
    Agent(#[from] crate::agent::error::AgentError),

    /// No API key for the organization's time platform: set one with
    /// `iter org key`, or `ITER_PLATFORM_KEY`.
    #[error("no API key for organization '{0}': run `iter org key {0}` or set ITER_PLATFORM_KEY")]
    MissingKey(String),

    /// The time platform could not be reached, or refused the request. The
    /// message has already been scrubbed of the API key.
    #[error("time platform: {0}")]
    Platform(String),

    #[error("no such {kind} '{name}'")]
    NotFound { kind: &'static str, name: String },

    /// `kind` is what the name belongs to: "board", "task", ...
    #[error("{0} name cannot be empty")]
    EmptyName(&'static str),

    #[error("no such task '{task}' in project '{project}'")]
    TaskNotFound { project: String, task: String },

    #[error("expected <project>/<task>, got '{0}'")]
    InvalidTaskRef(String),

    #[error("invalid date value '{0}', expected YYYY-MM-DD")]
    InvalidDate(String),

    #[error("--from {from} is after --to {to}")]
    InvalidDateRange { from: String, to: String },

    #[error("invalid status '{0}', expected queue, wip, or done")]
    InvalidStatus(String),

    #[error("invalid tag color '{0}' (expected #rrggbb)")]
    InvalidTagColor(String),

    #[error("tag '{0}' is built in and can't be renamed or deleted")]
    BuiltinTag(String),

    #[error("base path cannot be empty")]
    EmptyBasePath,

    /// Project and tag names travel comma-separated in the web forms'
    /// roster and tag fields, so a comma inside one can't round-trip.
    #[error("{kind} name '{name}' cannot contain a comma")]
    CommaInName { kind: &'static str, name: String },

    /// A project's name is the first half of `<project>/<task>`, so a slash
    /// in it would make that reference read back as a different task.
    #[error("{kind} name '{name}' cannot contain a slash")]
    SlashInName { kind: &'static str, name: String },

    /// A `UNIQUE` name refused on write (see `Db::insert`): `kind` is the
    /// entity, "project", "task", ... A task's name is unique within its
    /// project, the others' across the database.
    #[error("{} {kind} with this name already exists{}", article(.kind), name_scope(.kind))]
    NameTaken { kind: &'static str },

    /// Its worktree and branch live in the old project's repo.
    #[error("'{0}' has a session; tear it down before moving the task to another project")]
    TaskHasSession(String),

    #[error("'{0}' already has a session")]
    SessionAlreadyExists(String),

    #[error("project '{0}' has github disabled")]
    GithubDisabled(String),

    #[error("'{0}' has no linked github issue")]
    NoLinkedIssue(String),

    #[error("project '{name}' is marked github but {path} isn't a git repo")]
    NotAGitRepo { name: String, path: String },

    #[error("a tmux session named '{0}' already exists")]
    TmuxSessionExists(String),

    #[error("not inside a tmux session")]
    NotInTmux,

    #[error("tmux session '{0}' isn't tracked by iter")]
    UntrackedTmuxSession(String),

    /// A foreign key pointing at a row that's gone: the `what` a `of` names.
    #[error("{what} for this {of} no longer exists")]
    Orphan {
        what: &'static str,
        of: &'static str,
    },

    /// A project in no board/organization, where one was needed.
    /// `relation` reads as "isn't bound to a board".
    #[error("project '{project}' {relation}")]
    NotAMember {
        project: String,
        relation: &'static str,
    },

    #[error("no open session for '{0}' -- run `iter session start` or attach to its tmux session")]
    NoOpenSession(String),

    #[error(
        "'{0}' has no branch and worktree to save -- \
         --save merges the ones `iter session new` creates"
    )]
    NothingToSave(String),

    #[error(
        "{path} doesn't have '{branch}' checked out -- \
         --save merges into the branch that's checked out there, so check \
         out '{branch}' first"
    )]
    NotOnDefaultBranch { path: String, branch: String },

    #[error(
        "merging '{branch}' into '{into}' stopped -- git's output above \
         says what's in the way; finish it in {path}, then re-run"
    )]
    MergeStopped {
        branch: String,
        into: String,
        path: String,
    },
}

fn article(kind: &str) -> &'static str {
    if kind.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "an"
    } else {
        "a"
    }
}

/// Where a name must be unique, for [`IterError::NameTaken`]'s message.
fn name_scope(kind: &str) -> &'static str {
    if kind == "task" {
        " in this project"
    } else {
        ""
    }
}

/// A "ran, but failed" error with a message: the one-liner the commands share.
pub fn failed(message: impl Into<String>) -> IterError {
    IterError::CommandFailed(message.into())
}

pub type Result<T> = std::result::Result<T, IterError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parameterised_errors_read_as_the_per_entity_ones_did() {
        let cases = [
            (
                IterError::NotFound {
                    kind: "organization",
                    name: "acme".into(),
                },
                "no such organization 'acme'",
            ),
            (
                IterError::NotFound {
                    kind: "tag",
                    name: "x".into(),
                },
                "no such tag 'x'",
            ),
            (
                IterError::NameTaken { kind: "project" },
                "a project with this name already exists",
            ),
            (
                IterError::NameTaken {
                    kind: "organization",
                },
                "an organization with this name already exists",
            ),
            (
                IterError::NameTaken { kind: "task" },
                "a task with this name already exists in this project",
            ),
            (IterError::EmptyBasePath, "base path cannot be empty"),
            (IterError::EmptyName("board"), "board name cannot be empty"),
            (IterError::EmptyName("task"), "task name cannot be empty"),
            (
                IterError::Orphan {
                    what: "task",
                    of: "session",
                },
                "task for this session no longer exists",
            ),
            (
                IterError::Orphan {
                    what: "organization",
                    of: "project",
                },
                "organization for this project no longer exists",
            ),
            (
                IterError::NotAMember {
                    project: "p".into(),
                    relation: "isn't bound to a board",
                },
                "project 'p' isn't bound to a board",
            ),
        ];
        for (error, expected) in cases {
            assert_eq!(error.to_string(), expected);
        }
    }
}
