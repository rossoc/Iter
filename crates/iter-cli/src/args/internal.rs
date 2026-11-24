//! Internal subcommands: tmux hooks and the events they report, and the session's paste worker.

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

    /// Started detached by `session new` for a window with `paste` in
    /// `session.windows`: waits until the window's program runs, then pastes
    /// the prompt of `instruction` into it, without pressing Enter
    #[clap(disable_help_flag = true)]
    Paste {
        #[clap(allow_hyphen_values = true)]
        session: String,
        #[clap(allow_hyphen_values = true)]
        window: String,
        #[clap(allow_hyphen_values = true)]
        instruction: String,
    },
}

/// The tmux events `iter` tracks time on, each with exactly the arguments
/// that name the sessions it concerns. The names come from tmux and may
/// start with `-` (a project called `-web`): every positional takes hyphen
/// values (no `-h` help flag: only tmux calls these), and the hooks iter
/// installs also pass `--` before them.
#[derive(clap::Subcommand, Debug)]
pub enum HookEvent {
    /// `client` now shows `session`, having left `previous` (empty on a
    /// first attach)
    #[clap(disable_help_flag = true)]
    ClientSessionChanged {
        #[clap(allow_hyphen_values = true)]
        client: String,
        #[clap(allow_hyphen_values = true)]
        session: String,
        #[clap(allow_hyphen_values = true)]
        previous: Option<String>,
    },
    /// `client` is gone; which session it was on is what `iter` recorded
    /// the last time it changed
    #[clap(disable_help_flag = true)]
    ClientDetached {
        #[clap(allow_hyphen_values = true)]
        client: String,
    },
    /// `session` no longer exists
    #[clap(disable_help_flag = true)]
    SessionClosed {
        #[clap(allow_hyphen_values = true)]
        session: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::{Args, Command};
    use clap::Parser;

    fn event(argv: &[&str]) -> HookEvent {
        let args = Args::try_parse_from([&["iter", "internal", "hook"], argv].concat())
            .unwrap_or_else(|e| panic!("{argv:?}: {e}"));
        match args.command {
            Command::Internal {
                action: InternalCommand::Hook { event },
            } => event,
            other => panic!("{other:?}"),
        }
    }

    /// R4c of task 302: `session new` starts `iter internal paste <session> <window>
    /// <instruction>` detached; a session name may start with `-` and has spaces.
    #[test]
    fn the_paste_worker_takes_a_session_a_window_and_an_instruction() {
        for argv in [
            &[
                "iter",
                "internal",
                "paste",
                "web/1 login",
                "claude",
                "complete",
            ][..],
            &[
                "iter",
                "internal",
                "paste",
                "--",
                "-web/1 login",
                "claude",
                "complete",
            ],
        ] {
            let args = Args::try_parse_from(argv).unwrap_or_else(|e| panic!("{argv:?}: {e}"));
            let Command::Internal {
                action:
                    InternalCommand::Paste {
                        session,
                        window,
                        instruction,
                    },
            } = args.command
            else {
                panic!("{argv:?}")
            };
            assert_eq!(
                (session.as_str(), window.as_str(), instruction.as_str()),
                (argv[argv.len() - 3], "claude", "complete")
            );
        }
    }

    /// A session named after a project `-web` reached clap as a flag: exit 2, and that task's clock
    /// never started or stopped. Hooks installed before the `--` still pass the bare name.
    #[test]
    fn a_session_name_starting_with_a_dash_is_a_value() {
        for argv in [
            &["session-closed", "-web/a"][..],
            &["session-closed", "--", "-web/a"],
            &["session-closed", "-h"],
        ] {
            let HookEvent::SessionClosed { session } = event(argv) else {
                panic!("{argv:?}")
            };
            assert_eq!(session, argv[argv.len() - 1]);
        }
        let HookEvent::ClientSessionChanged {
            client,
            session,
            previous,
        } = event(&[
            "client-session-changed",
            "--",
            "/dev/ttys1",
            "-web/a",
            "-api/b",
        ])
        else {
            panic!()
        };
        assert_eq!(
            (client.as_str(), session.as_str(), previous.as_deref()),
            ("/dev/ttys1", "-web/a", Some("-api/b"))
        );
    }
}
