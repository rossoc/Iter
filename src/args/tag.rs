//! Tag subcommands.

use super::*;

#[derive(clap::Subcommand, Debug)]
pub enum TagCommand {
    /// Open a blank tag in nvim; saving & quitting creates it
    New,

    /// Edit an existing tag in nvim; saving & quitting updates it
    Edit {
        #[arg(add = ArgValueCompleter::new(tag_completer))]
        name: String,
    },

    /// Delete a tag; it's removed from every task that had it
    Delete {
        #[arg(add = ArgValueCompleter::new(tag_completer))]
        name: String,
    },

    /// A tag's color and description
    Info {
        #[arg(add = ArgValueCompleter::new(tag_completer))]
        name: String,
    },

    /// List every tag
    List,
}
