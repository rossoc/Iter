//! Board subcommands.

use super::*;

#[derive(clap::Subcommand, Debug)]
pub enum BoardCommand {
    /// Open a blank board in nvim; saving & quitting creates it
    New,

    /// Edit an existing board in nvim; saving & quitting updates it
    Edit {
        /// Default: the board of the tmux session's project
        #[arg(add = ArgValueCompleter::new(board_completer))]
        name: Option<String>,
    },

    /// Delete a board; its projects are kept, no longer bound to one
    Delete {
        /// Default: the board of the tmux session's project
        #[arg(add = ArgValueCompleter::new(board_completer))]
        name: Option<String>,
    },

    /// A board's description and the projects bound to it
    Info {
        /// Default: the board of the tmux session's project
        #[arg(add = ArgValueCompleter::new(board_completer))]
        name: Option<String>,
    },

    /// List every board
    List,
}
