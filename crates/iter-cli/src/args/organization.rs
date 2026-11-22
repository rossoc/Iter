//! Organization subcommands.

use super::*;

#[derive(clap::Subcommand, Debug)]
pub enum OrganizationCommand {
    /// Open a blank organization in nvim; saving & quitting creates it
    New,

    /// Edit an existing organization (and which projects are in it) in
    /// nvim; saving & quitting updates it
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

    /// Set (read from stdin) or clear the API key of the organization's
    /// time platform; it is kept in the OS keychain, never in the database
    Key {
        /// Default: the organization of the tmux session's project
        #[arg(add = ArgValueCompleter::new(organization_completer))]
        name: Option<String>,

        /// Remove the stored key instead of setting one
        #[clap(long)]
        clear: bool,
    },

    /// Push the worklog of a day or an interval (default: today) to the
    /// organization's time platform, and close and comment on the GitHub
    /// issues of the tasks completed in it
    Push {
        /// Default: the organization of the tmux session's project
        #[arg(add = ArgValueCompleter::new(organization_completer))]
        name: Option<String>,

        #[clap(flatten)]
        report: ReportOpts,

        /// Show what would be sent and done, and send nothing
        #[clap(long)]
        dry_run: bool,

        /// Push even if this exact worklog was pushed before, without asking.
        /// Days the platform already has an entry for are never pushed
        #[clap(long)]
        force: bool,

        /// Log under this platform organization instead of the default
        #[clap(long)]
        platform_organization: Option<String>,

        /// Log under this activity instead of the default
        #[clap(long)]
        activity: Option<String>,
    },

    /// List every organization
    List,
}
