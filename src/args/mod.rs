use crate::agent::prompt::Kind as PromptKind;
use crate::agent::run::Harness;
use crate::completion::{
    board_completer, organization_completer, project_completer, queued_task_completer,
    tag_completer, task_completer, task_name_completer,
};
use crate::reporting::Format;
use crate::utils::output::Format as DataFormat;
use crate::wiki::retrieve::Depth;
use clap::Parser;
use clap_complete::engine::ArgValueCompleter;

mod board;
mod internal;
mod notes;
mod organization;
mod project;
mod session;
mod tag;
mod task;
mod wiki;

pub use board::*;
pub use internal::*;
pub use notes::*;
pub use organization::*;
pub use project::*;
pub use session::*;
pub use tag::*;
pub use task::*;
pub use wiki::*;

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

        /// Bind the new project to this board (when cloning a project, the
        /// clone otherwise lands on that project's board, if any)
        #[clap(long, short = 'b', add = ArgValueCompleter::new(board_completer))]
        board: Option<String>,

        /// Leave the new project on no board, even if the project it was
        /// cloned from is on one
        #[clap(long, conflicts_with = "board")]
        no_board: bool,
    },

    /// Manage tags: coloured labels shared by every task
    Tag {
        #[clap(subcommand)]
        action: TagCommand,
    },

    /// Manage boards: calendars that hold projects
    Board {
        #[clap(subcommand)]
        action: BoardCommand,
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

    /// Read the wiki built from the notes: find the pages worth loading,
    /// follow their relations, query the graph, keep it consistent
    Wiki {
        #[clap(subcommand)]
        action: WikiCommand,
    },

    /// Daily notes of a project or an organization
    Notes {
        #[clap(subcommand)]
        action: NotesCommand,
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

    /// Browse and edit organizations, projects and tasks in the browser
    /// (a local server over the same database the CLI uses)
    #[cfg(feature = "web")]
    Serve {
        /// Port to listen on (127.0.0.1 only)
        #[clap(long, short = 'p', default_value_t = 3000)]
        port: u16,

        /// Don't open the browser
        #[clap(long)]
        no_open: bool,
    },

    /// Commands invoked by tmux hooks -- not meant to be run by hand
    #[clap(hide = true)]
    Internal {
        #[clap(subcommand)]
        action: InternalCommand,
    },
}
