//! Notes subcommands.

use super::*;

/// Which notes: a project's or an organization's (default: the project of the
/// tmux session you're in).
#[derive(clap::Args, Debug)]
pub struct NotesScope {
    #[clap(long, short = 'p', add = ArgValueCompleter::new(project_completer),
           conflicts_with = "org")]
    pub project: Option<String>,

    #[clap(long = "org", short = 'o', add = ArgValueCompleter::new(organization_completer))]
    pub org: Option<String>,
}

#[derive(clap::Subcommand, Debug)]
pub enum NotesCommand {
    /// Make today's note if it is not there and print its path
    Today {
        #[clap(flatten)]
        scope: NotesScope,
    },

    /// List the notes, latest first
    List {
        #[clap(flatten)]
        scope: NotesScope,
    },

    /// Print a note (default: today's)
    Show {
        /// YYYY-MM-DD
        date: Option<String>,

        #[clap(flatten)]
        scope: NotesScope,
    },
}
