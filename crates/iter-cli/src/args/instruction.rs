//! Instruction subcommands.

use super::*;

#[derive(clap::Subcommand, Debug)]
pub enum InstructionCommand {
    /// The instructions there are for a task (default: the current one; else
    /// the current project's, else iter's config folder's), one per line:
    /// name, where it comes from (task, project, org or config), description
    List {
        /// `<project>/<task>` whose instructions to list
        #[clap(long, short = 't', add = ArgValueCompleter::new(task_completer))]
        task: Option<String>,
    },

    /// Print an instruction: the nearest file of that name
    Show {
        name: String,

        /// `<project>/<task>` whose instructions to look in
        #[clap(long, short = 't', add = ArgValueCompleter::new(task_completer))]
        task: Option<String>,
    },
}
