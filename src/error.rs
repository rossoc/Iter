use std::path::PathBuf;
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
    Agent(#[from] AgentError),

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

    #[error("{} tasks in '{project}' are titled '{title}': {} -- name one by its id", .ids.len(), ids_list(.ids))]
    AmbiguousTask {
        project: String,
        title: String,
        ids: Vec<i64>,
    },

    /// The database could not be opened. The message is the whole line, so `main` prints it
    /// without an `error: ` prefix.
    #[error("failed to open database at {}: {source}", .path.display())]
    OpenDb {
        path: PathBuf,
        source: Box<IterError>,
    },

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

    /// A clone copying a project's folder into a folder inside it would copy its own copy.
    #[error("cannot clone into {dest}: it is inside the source folder {src}")]
    CloneIntoSource { src: String, dest: String },

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

    #[error("no current task: name one, or run this inside its worktree or its tmux session")]
    NoCurrentTask,

    #[error(
        "no current project: name one, or run this inside its folder, a task's worktree or tmux session"
    )]
    NoCurrentProject,

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

/// `#3, #9, #12`: the ids of [`IterError::AmbiguousTask`].
fn ids_list(ids: &[i64]) -> String {
    ids.iter()
        .map(|id| format!("#{id}"))
        .collect::<Vec<_>>()
        .join(", ")
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

/// Errors of the agent workflow; they convert into [`IterError`] with `?` and print unchanged.
pub type AgentResult<T> = std::result::Result<T, AgentError>;

#[derive(Debug, Error)]
#[cfg_attr(not(feature = "web"), allow(dead_code))] // the instruction editor (web only) raises most variants
pub enum AgentError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("no such run")]
    RunNotFound(u32),
    #[error("the run vanished")]
    RunVanished(u32),
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
    #[error("run {0} was started by iter, not for a subagent: it closes itself")]
    RunNotSubagent(u32),
    #[error("run {run} is {status}: only a running subagent's run can be closed")]
    RunNotOpen { run: u32, status: String },
    #[error(
        "{0} leads subagents from a session and is not a run: print it with `iter task prompt -i {0}`"
    )]
    Interactive(String),
    #[error(
        "no instruction '{name}' in the task, the project's or the organization's notes, or {}: write it there",
        .path.display()
    )]
    InstructionNotFound { name: String, path: PathBuf },
    #[error("{}: {problem}", .path.display())]
    InstructionUnreadable { path: PathBuf, problem: String },
    #[error("no such task")]
    TaskGone,
    #[error("no such project")]
    ProjectGone,
    #[error("{0} is not a git repository")]
    NotAGitRepo(String),

    #[error("the buffer lost its front matter (the lines between `---`)")]
    BufferLostFront,
    #[error("front matter: {0}")]
    BufferFront(serde_yaml::Error),

    #[error("the plan is empty: nothing was saved")]
    EmptyPlan,
    #[error("there is no plan yet: write one with `iter task plan`")]
    NoPlan,
    #[error("{0}")]
    NoSuchStep(#[from] MarkError),
    #[error("`{0}` is not a state: todo, doing or done")]
    UnknownState(String),

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
    #[error("front matter: `{0}` must be true or false")]
    FrontMatterNotFlag(String),
    #[error("'{0}' is not a name: lowercase letters, digits and dashes")]
    InvalidName(String),
    #[error("an instruction needs a prompt under its front matter")]
    NoPrompt,
    #[error("permission: {0}")]
    FieldPermission(serde_yaml::Error),
    #[error("{0}")]
    Yaml(serde_yaml::Error),

    #[error("an agent is already writing in this wiki")]
    WikiBusy,
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

#[derive(Debug, PartialEq)]
pub enum MarkError {
    NoSuchStep(String, Vec<String>),
}

impl std::fmt::Display for MarkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let MarkError::NoSuchStep(id, ids) = self;
        match ids.is_empty() {
            true => write!(f, "no step '{id}': the plan has no steps"),
            false => write!(f, "no step '{id}' (the steps are {})", ids.join(", ")),
        }
    }
}

impl std::error::Error for MarkError {}

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

    /// R2: the error lists the `#<id>` of every match, so the user can name one.
    #[test]
    fn an_ambiguous_title_lists_the_ids_to_choose_from() {
        let error = IterError::AmbiguousTask {
            project: "web".into(),
            title: "login".into(),
            ids: vec![3, 9, 12],
        };
        assert_eq!(
            error.to_string(),
            "3 tasks in 'web' are titled 'login': #3, #9, #12 -- name one by its id"
        );
    }

    /// Messages are shown verbatim by the CLI and the pages.
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
