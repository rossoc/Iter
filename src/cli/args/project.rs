//! Project subcommands.

use super::*;

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

        /// Board to bind the project to (optional)
        #[clap(long, short = 'b', add = ArgValueCompleter::new(board_completer))]
        board: Option<String>,
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

        /// Bind the project to this board (its current board, if any, is
        /// kept when this isn't given)
        #[clap(long, short = 'b', add = ArgValueCompleter::new(board_completer))]
        board: Option<String>,

        /// Unbind the project from its board
        #[clap(long, conflicts_with = "board")]
        no_board: bool,
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
