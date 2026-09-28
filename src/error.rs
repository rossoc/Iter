use thiserror::Error;

/// The one error type for the whole app. External-tool failures (`git`,
/// `tmux`, `gh`, `nvim`) are split into "couldn't even run it" (`Spawn`,
/// carrying the actual `io::Error`) and "ran, but failed" (`CommandFailed`,
/// a plain message built at the call site, since a nonzero exit status
/// doesn't carry a structured error of its own).
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

    #[error("failed to run `{tool}`: {source}")]
    Spawn {
        tool: &'static str,
        #[source]
        source: std::io::Error,
    },

    #[error("{0}")]
    CommandFailed(String),

    /// A board, organization, project or tag looked up by a name nothing has.
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

    #[error("base_path cannot be empty")]
    EmptyBasePath,

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

pub type Result<T> = std::result::Result<T, IterError>;

#[cfg(test)]
mod tests {
    use super::*;

    /// The parameterised variants replaced one variant per entity; what
    /// the user reads has to be exactly what those used to say.
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
