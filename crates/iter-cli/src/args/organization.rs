//! Organization subcommands.

use super::*;

/// An organization named on the command line, else the one of the tmux session's project.
#[derive(clap::Args, Debug)]
pub struct OrgRef {
    /// Default: the organization of the tmux session's project
    #[arg(add = ArgValueCompleter::new(organization_completer))]
    name: Option<String>,
}

impl OrgRef {
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum OrganizationCommand {
    /// Open a blank organization in nvim; saving & quitting creates it
    New,

    /// Edit an existing organization (and which projects are in it) in
    /// nvim; saving & quitting updates it
    Edit {
        #[clap(flatten)]
        name: OrgRef,
    },

    /// Delete an organization; its projects are kept, no longer in one
    Delete {
        #[clap(flatten)]
        name: OrgRef,
    },

    /// Time spent across the organization over a day or an interval, broken
    /// down per project and per task
    Info {
        #[clap(flatten)]
        name: OrgRef,

        #[clap(flatten)]
        report: ReportOpts,
    },

    /// Set (read from stdin) or clear the API key of the organization's
    /// time platform; it is kept in the OS keychain, never in the database
    Key {
        #[clap(flatten)]
        name: OrgRef,

        /// Remove the stored key instead of setting one
        #[clap(long)]
        clear: bool,
    },

    /// Push the worklog of a day or an interval (default: today) to the
    /// organization's time platform, and close and comment on the GitHub
    /// issues of the tasks completed in it
    Push(OrgPushArgs),

    /// List every organization
    List,
}

#[derive(clap::Args, Debug)]
pub struct OrgPushArgs {
    #[clap(flatten)]
    pub name: OrgRef,

    #[clap(flatten)]
    pub report: ReportOpts,

    /// Show what would be sent and done, and send nothing
    #[clap(long)]
    pub dry_run: bool,

    /// Push even if this exact worklog was pushed before, without asking.
    /// Days the platform already has an entry for are never pushed
    #[clap(long)]
    pub force: bool,

    /// Log under this platform organization instead of the default
    #[clap(long)]
    pub platform_organization: Option<String>,

    /// Log under this activity instead of the default
    #[clap(long)]
    pub activity: Option<String>,
}
