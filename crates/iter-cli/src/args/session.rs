//! Session subcommands.

use super::*;

#[derive(clap::Subcommand, Debug)]
pub enum SessionCommand {
    /// Set up a session for a task: creates a git worktree/branch (unless
    /// the project has no github or auto_branch is off) and, if the
    /// project has tmux enabled, a tmux session named `<project>/<task>`
    /// with the windows of `session.windows` in iter.yaml (by default a
    /// shell, and `claude` with the `complete` prompt pasted in, unsent)
    New {
        /// `<project>/<task>` (completion offers only `queue` tasks)
        #[arg(add = ArgValueCompleter::new(queued_task_completer))]
        task: String,

        /// Override the branch name (default: the task's branch_prefix
        /// followed by its slugified name)
        #[clap(short = 'b', long = "branch")]
        branch: Option<String>,

        /// Skip branch/worktree creation entirely
        #[clap(long = "no-branch")]
        no_branch: bool,
    },

    /// Manually start a session for a task (for tmux-less projects, or when
    /// the tmux hooks aren't in play)
    Start {
        #[clap(flatten)]
        task: TaskRef,
    },

    /// Manually end the open session for a task
    Stop {
        #[clap(flatten)]
        task: TaskRef,

        /// Note stored alongside the closed session
        #[clap(short = 'm', long = "message")]
        message: Option<String>,
    },

    /// Time spent so far on the currently open session of the task of the
    /// tmux session you're in
    Elapse,
}
