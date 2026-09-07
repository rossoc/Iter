use crate::{organization_completer, project_completer, task_completer};
use clap::Parser;
use clap_complete::engine::ArgValueCompleter;

/// How an `info` report is printed.
#[derive(clap::ValueEnum, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Format {
    /// A readable markdown report: YAML front matter, a `---` divider, then
    /// the description and a breakdown of the period's sessions
    #[default]
    Text,
    /// The same report as plain YAML, for piping somewhere else
    Yaml,
}

/// The period an `info` report covers, and how it's printed.
///
/// Two mutually exclusive ways to say which days: `--date` for a single one
/// (the default, today), or `--from`/`--to` for an interval. Either bound of
/// the interval may be left off -- `--from` alone runs up to today, `--to`
/// alone reaches back over every session there is.
#[derive(clap::Args, Debug)]
pub struct ReportOpts {
    /// A single day to report on, YYYY-MM-DD (default: today)
    #[clap(long, conflicts_with_all = ["from", "to"])]
    pub date: Option<String>,

    /// First day of an interval, YYYY-MM-DD (default: no lower bound)
    #[clap(long)]
    pub from: Option<String>,

    /// Last day of an interval, YYYY-MM-DD (default: today)
    #[clap(long)]
    pub to: Option<String>,

    /// Output format
    #[clap(long, short = 'f', value_enum, default_value_t = Format::Text)]
    pub format: Format,
}

/// A project/task/session time tracker, with tmux + GitHub integration.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Args {
    #[clap(subcommand)]
    pub command: Command,
}

#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Create a project from the current directory (cwd becomes base_path)
    Init {
        /// Organization to put the project in; its settings become the
        /// project's starting defaults (optional -- a project need not
        /// belong to one)
        #[clap(long = "organization", short = 'o',
               add = ArgValueCompleter::new(organization_completer))]
        organization: Option<String>,
    },

    /// Create a project from scratch in a new folder
    New {
        /// Directory to create; becomes the project's base_path
        path: String,

        /// Organization to put the project in; its settings become the
        /// project's starting defaults (optional -- a project need not
        /// belong to one)
        #[clap(long = "organization", short = 'o',
               add = ArgValueCompleter::new(organization_completer))]
        organization: Option<String>,
    },

    /// Create a project from a template project (copies its fields) or a
    /// git/GitHub repo (like `git clone`)
    Clone {
        /// An existing project's name, or a git remote URL / local repo
        /// path if no such project exists
        #[arg(add = ArgValueCompleter::new(project_completer))]
        source: String,

        /// Organization to put the project in; its settings become the
        /// project's starting defaults (optional -- a project need not
        /// belong to one)
        #[clap(long = "organization", short = 'o',
               add = ArgValueCompleter::new(organization_completer))]
        organization: Option<String>,
    },

    /// Manage organizations: groups of projects that share defaults
    #[clap(visible_alias = "org")]
    Organization {
        #[clap(subcommand)]
        action: OrganizationCommand,
    },

    /// Manage projects
    Project {
        #[clap(subcommand)]
        action: ProjectCommand,
    },

    /// Manage tasks
    Task {
        #[clap(subcommand)]
        action: TaskCommand,
    },

    /// Set up and track a task's session (tmux + git worktree/branch, and
    /// time spent)
    Session {
        #[clap(subcommand)]
        action: SessionCommand,
    },

    /// Post a message as a comment on a task's linked GitHub issue
    Comment {
        /// `<project>/<task>` (default: the task of the tmux session you're in)
        #[arg(add = ArgValueCompleter::new(task_completer))]
        task: Option<String>,

        /// The comment body
        #[clap(short = 'm', long = "message")]
        message: String,
    },

    /// Print `<project>/<task>` for the tmux session you're currently in
    T,

    /// Commands invoked by tmux hooks -- not meant to be run by hand
    #[clap(hide = true)]
    Internal {
        #[clap(subcommand)]
        action: InternalCommand,
    },
}

#[derive(clap::Subcommand, Debug)]
pub enum ProjectCommand {
    /// Open a blank project in nvim; saving & quitting creates it
    New {
        /// Organization to put the project in; its settings become the
        /// project's starting defaults (optional -- a project need not
        /// belong to one)
        #[clap(long = "organization", short = 'o',
               add = ArgValueCompleter::new(organization_completer))]
        organization: Option<String>,
    },

    /// Edit an existing project in nvim; saving & quitting updates it
    Edit {
        /// Default: the project of the tmux session you're in
        #[arg(add = ArgValueCompleter::new(project_completer))]
        name: Option<String>,

        /// Move the project into this organization (its current
        /// organization, if any, is kept when this isn't given)
        #[clap(long = "organization", short = 'o',
               add = ArgValueCompleter::new(organization_completer))]
        organization: Option<String>,
    },

    /// Delete a project (and its tasks, session-configs and sessions)
    Delete {
        /// Default: the project of the tmux session you're in
        #[arg(add = ArgValueCompleter::new(project_completer))]
        name: Option<String>,
    },

    /// Name, description, and time spent (union of all its tasks) over a
    /// day or an interval, broken down per task
    Info {
        /// Default: the project of the tmux session you're in
        #[arg(add = ArgValueCompleter::new(project_completer))]
        name: Option<String>,

        #[clap(flatten)]
        report: ReportOpts,
    },

    /// List every project
    List,
}

#[derive(clap::Subcommand, Debug)]
pub enum OrganizationCommand {
    /// Open a blank organization in nvim; saving & quitting creates it
    New,

    /// Edit an existing organization in nvim; saving & quitting updates it
    Edit {
        /// Default: the organization of the tmux session's project
        #[arg(add = ArgValueCompleter::new(organization_completer))]
        name: Option<String>,
    },

    /// Delete an organization; its projects are kept, no longer in one
    Delete {
        /// Default: the organization of the tmux session's project
        #[arg(add = ArgValueCompleter::new(organization_completer))]
        name: Option<String>,
    },

    /// Time spent across the organization over a day or an interval, broken
    /// down per project and per task
    Info {
        /// Default: the organization of the tmux session's project
        #[arg(add = ArgValueCompleter::new(organization_completer))]
        name: Option<String>,

        #[clap(flatten)]
        report: ReportOpts,
    },

    /// List every organization
    List,
}

#[derive(clap::Subcommand, Debug)]
pub enum TaskCommand {
    /// Open a new task in nvim (optionally pre-filled from a GitHub issue);
    /// saving & quitting creates it
    New {
        /// Project the task belongs to (default: the project of the tmux
        /// session you're in)
        #[clap(long, add = ArgValueCompleter::new(project_completer))]
        project: Option<String>,

        /// Pre-fill name/description/github_issue from this issue number
        #[clap(long)]
        issue: Option<i64>,
    },

    /// Edit an existing task in nvim; saving & quitting updates it
    Edit {
        /// `<project>/<task>` (default: the task of the tmux session you're in)
        #[arg(add = ArgValueCompleter::new(task_completer))]
        task: Option<String>,
    },

    /// Delete a task (and its session-config and sessions)
    Delete {
        /// `<project>/<task>` (default: the task of the tmux session you're in)
        #[arg(add = ArgValueCompleter::new(task_completer))]
        task: Option<String>,
    },

    /// Name, description, and every session on a day or over an interval
    /// (default: today)
    Info {
        /// `<project>/<task>` (default: the task of the tmux session you're in)
        #[arg(add = ArgValueCompleter::new(task_completer))]
        task: Option<String>,

        #[clap(flatten)]
        report: ReportOpts,
    },

    /// List tasks, optionally filtered by project and/or status
    List {
        #[clap(long, add = ArgValueCompleter::new(project_completer))]
        project: Option<String>,

        /// queue, wip, or done
        #[clap(long)]
        status: Option<String>,
    },

    /// Mark a task done: closes any open session, tears down its
    /// session-config (tmux session + worktree), and deletes that row
    Done {
        /// `<project>/<task>` (default: the task of the tmux session you're in)
        #[arg(add = ArgValueCompleter::new(task_completer))]
        task: Option<String>,
    },

    /// Average hours per weekday spent on a task
    Weekday {
        /// `<project>/<task>` (default: the task of the tmux session you're in)
        #[arg(add = ArgValueCompleter::new(task_completer))]
        task: Option<String>,
    },
}

#[derive(clap::Subcommand, Debug)]
pub enum SessionCommand {
    /// Set up a session for a task: creates a git worktree/branch (unless
    /// the project has no github or auto_branch is off) and, if the
    /// project has tmux enabled, a tmux session named `<project>/<task>`
    New {
        #[arg(add = ArgValueCompleter::new(task_completer))]
        task: String,

        /// Override the branch name (default: the project's branch_template)
        #[clap(short = 'b', long = "branch")]
        branch: Option<String>,

        /// Skip branch/worktree creation entirely
        #[clap(long = "no-branch")]
        no_branch: bool,
    },

    /// Manually start a session for a task (for tmux-less projects, or when
    /// the tmux hooks aren't in play)
    Start {
        /// `<project>/<task>` (default: the task of the tmux session you're in)
        #[arg(add = ArgValueCompleter::new(task_completer))]
        task: Option<String>,
    },

    /// Manually end the open session for a task
    Stop {
        /// `<project>/<task>` (default: the task of the tmux session you're in)
        #[arg(add = ArgValueCompleter::new(task_completer))]
        task: Option<String>,

        /// Note stored alongside the closed session
        #[clap(short = 'm', long = "message")]
        message: Option<String>,
    },

    /// Time spent so far on the currently open session of the task of the
    /// tmux session you're in
    Elapse,
}

#[derive(clap::Subcommand, Debug)]
pub enum InternalCommand {
    /// Called by a tmux hook: event is client-attached / client-detached /
    /// session-closed, tmux_session is the tmux session name (#{hook_session_name})
    Hook { event: String, tmux_session: String },
}
