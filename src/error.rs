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

    #[error("failed to run `{tool}`: {source}")]
    Spawn {
        tool: &'static str,
        #[source]
        source: std::io::Error,
    },

    #[error("{0}")]
    CommandFailed(String),

    #[error("no such project '{0}'")]
    ProjectNotFound(String),

    #[error("no such task '{task}' in project '{project}'")]
    TaskNotFound { project: String, task: String },

    #[error("expected <project>/<task>, got '{0}'")]
    InvalidTaskRef(String),

    #[error("invalid --date value '{0}', expected YYYY-MM-DD")]
    InvalidDate(String),

    #[error("invalid status '{0}', expected queue, wip, or done")]
    InvalidStatus(String),

    #[error("project name cannot be empty")]
    EmptyProjectName,

    #[error("task name cannot be empty")]
    EmptyTaskName,

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

    #[error("task for this session no longer exists")]
    OrphanSessionTask,

    #[error("project for this task no longer exists")]
    OrphanTaskProject,

    #[error("no open session for '{0}' -- run `iter session start` or attach to its tmux session")]
    NoOpenSession(String),
}

pub type Result<T> = std::result::Result<T, IterError>;
