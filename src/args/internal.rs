//! Internal subcommands: tmux hooks and the events they report.

#[derive(clap::Subcommand, Debug)]
pub enum InternalCommand {
    /// Called by a tmux hook; see `tmux::HOOKS` for which format variable
    /// fills which argument
    Hook {
        #[clap(subcommand)]
        event: HookEvent,
    },

    /// Stops the clock of every task with a tmux session, then kills the
    /// tmux server -- tmux has no hook of its own for the server exiting
    KillServer,

    /// Re-links iter's sessions to a freshly started tmux server: recreates
    /// the tmux sessions it lacks, installs the hooks, closes the clocks the
    /// previous server left open and starts those of attached sessions.
    /// Meant for tmux-resurrect's `@resurrect-hook-post-restore-all`
    Restore,
}

/// The tmux events `iter` tracks time on, each with exactly the arguments
/// that name the sessions it concerns.
#[derive(clap::Subcommand, Debug)]
pub enum HookEvent {
    /// `client` now shows `session`, having left `previous` (empty on a
    /// first attach)
    ClientSessionChanged {
        client: String,
        session: String,
        previous: Option<String>,
    },
    /// `client` is gone; which session it was on is what `iter` recorded
    /// the last time it changed
    ClientDetached { client: String },
    /// `session` no longer exists
    SessionClosed { session: String },
}
