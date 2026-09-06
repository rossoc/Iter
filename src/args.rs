use crate::session_name_completer;
use clap::Parser;
use clap_complete::engine::ArgValueCompleter;

/// A simple CLI for time-tracking named tasks: begin, end, info, elapsed,
/// list, and session-reporting commands.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Args {
    #[clap(subcommand)]
    pub command: Command,
}

#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Begin a task with a name
    Begin {
        #[arg(add = ArgValueCompleter::new(session_name_completer))]
        value: String,
    },

    /// End a task with a name
    End {
        #[arg(add = ArgValueCompleter::new(session_name_completer))]
        value: String,

        /// Optional note stored alongside the end event
        #[clap(short = 'm', long = "message")]
        message: Option<String>,
    },

    /// Total time and messages for one session name on one day (default: today)
    Info {
        #[arg(add = ArgValueCompleter::new(session_name_completer))]
        value: String,

        #[clap(long)]
        date: Option<String>,
    },

    /// Time elapsed since a task's last begin
    Elapsed {
        #[arg(add = ArgValueCompleter::new(session_name_completer))]
        value: String,
    },

    /// List the distinct session names that have been logged
    List,

    /// Report on logged sessions, in YAML
    Session {
        #[clap(subcommand)]
        action: SessionCommand,
    },
}

#[derive(clap::Subcommand, Debug)]
pub enum SessionCommand {
    /// Average hours per weekday for one name
    Weekday {
        #[arg(add = ArgValueCompleter::new(session_name_completer))]
        value: String,
    },
}
