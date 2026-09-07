use crate::{organization_completer, project_completer, task_completer};
use clap::Parser;
use clap_complete::engine::ArgValueCompleter;

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

    /// Manage projects; with no subcommand, lists every project by name
    Project {
        #[clap(subcommand)]
        action: Option<ProjectCommand>,
    },

    /// Manage tasks; with no subcommand, lists every unfinished (queue or
    /// wip) task
    Task {
        #[clap(subcommand)]
        action: Option<TaskCommand>,
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

    /// Name, description, and time spent (union of all its tasks) on a day
    Info {
        /// Default: the project of the tmux session you're in
        #[arg(add = ArgValueCompleter::new(project_completer))]
        name: Option<String>,

        #[clap(long)]
        date: Option<String>,
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

    /// Every project in the organization, with each project's tasks and
    /// their status
    Info {
        /// Default: the organization of the tmux session's project
        #[arg(add = ArgValueCompleter::new(organization_completer))]
        name: Option<String>,
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

    /// Name, description, and time spent on a day (default: today)
    Info {
        /// `<project>/<task>` (default: the task of the tmux session you're in)
        #[arg(add = ArgValueCompleter::new(task_completer))]
        task: Option<String>,

        #[clap(long)]
        date: Option<String>,
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

        /// Override the branch name (default: the task's branch_prefix
        /// followed by its slugified name)
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
    /// Called by a tmux hook. `event` is client-session-changed /
    /// client-detached / session-closed; `tmux_session` is the session it
    /// fired for, and `previous` -- only sent for client-session-changed --
    /// is the session the client just left. See `tmux::HOOKS` for which
    /// format variable names the session on which event.
    Hook {
        event: String,
        tmux_session: String,
        previous: Option<String>,
    },
}
