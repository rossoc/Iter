use crate::error::Result;
use crate::process::{Tmux, Tool};

/// Creates a new detached tmux session named `name`, starting in `cwd`.
pub fn create_session(name: &str, cwd: &str) -> Result<()> {
    Tmux.run(&["new-session", "-d", "-s", name, "-c", cwd])
}

/// Kills a tmux session if it's running. Not an error if it's already gone.
pub fn kill_session(name: &str) {
    let _ = Tmux.run(&["kill-session", "-t", name]);
}

/// Whether this process is itself running inside a tmux client -- which
/// decides how `attach_session` gets the user there: a client can't attach
/// to a second session from within one, it switches to it instead.
fn inside_tmux() -> bool {
    std::env::var_os("TMUX").is_some()
}

/// Puts the user in the session named `name`: attaching (this blocks,
/// handing the terminal to tmux until the user detaches) or, when we're
/// already inside tmux, switching the current client over to it.
pub fn attach_session(name: &str) -> Result<()> {
    let verb = if inside_tmux() {
        "switch-client"
    } else {
        "attach-session"
    };
    Tmux.run(&[verb, "-t", name])
}

pub fn session_exists(name: &str) -> bool {
    Tmux.succeeds(&["has-session", "-t", name])
}

/// Whether any client is still attached to `session`.
///
/// One tmux session can have several clients on it -- a second terminal, or
/// someone pairing -- and each of them detaches separately. Without this,
/// the first detach closes the task's session while another client is still
/// working in it, and every minute after that goes unrecorded.
///
/// This is only usable from a hook because of *when* the hook runs.
/// Measured on tmux 3.6: by the time `client-detached` fires, the client
/// that left is already off the list, so what this counts is exactly the
/// clients that remain. (`#{client_name}`, which would be the obvious way
/// to name the one leaving, is no help -- on `client-detached` it names a
/// *surviving* client, the same kind of trap [`HOOKS`] documents for
/// `#{hook_session_name}`.)
///
/// Any failure is a `false` -- no tmux, or a session that's already gone,
/// which is exactly the `session-closed` case -- so the caller closes the
/// session, which is what it did before this existed.
pub fn any_client_attached(session: &str) -> bool {
    let Ok(clients) = Tmux.output(&["list-clients", "-t", session, "-F", "#{client_name}"]) else {
        return false;
    };
    !clients.trim().is_empty()
}

/// The tmux session name of the pane this process is running in, if any
/// (i.e. we're inside a tmux client). Used by `iter t`.
pub fn current_session_name() -> Option<String> {
    // Outside tmux the answer is already known, and asking costs a
    // fork+exec on every command that defaults to "the task I'm in".
    if !inside_tmux() {
        return None;
    }
    Tmux.output_trimmed(&["display-message", "-p", "#S"])
}

/// The session-scoped tmux option a detach message is left in: something
/// bound to `detach-client` sets it just before detaching, and the
/// `client-detached` hook takes it back off again and stores it on the
/// session it closes. Named as it is because it's a shell-visible
/// interface, not an internal one -- a `bind` in `tmux.conf` writes it.
pub const DETACH_MESSAGE_OPTION: &str = "@iter_detach_message";

/// The hooks `iter` drives, each with the format string naming the session
/// it fired for -- and, for `client-session-changed`, the session being
/// left.
///
/// The variable differs per event, and getting it wrong fails *silently*:
/// the hook still runs, it just names no session `iter` knows, so the
/// handler ignores it. Measured on tmux 3.6:
///
/// | event                   | `hook_session_name` | `session_name` |
/// |-------------------------|---------------------|----------------|
/// | `client-session-changed` | empty               | the new session|
/// | `client-detached`        | empty               | the session    |
/// | `session-closed`         | the session         | *another one*  |
///
/// So `#{session_name}` is the only usable one for the client hooks, and
/// on `session-closed` it's actively wrong -- it names whatever session the
/// client is on now, not the one that just closed.
///
/// `client-session-changed` rather than `client-attached` is what starts a
/// session: it fires on a plain attach *and* on `switch-client`, so hopping
/// between two tasks inside tmux is seen. `client-attached` fires only on
/// the former, and would leave the task you switched away from running.
///
/// Which client fired the hook is deliberately *not* passed: `#{client_name}`
/// doesn't name the one that left (see [`any_client_attached`], which is
/// how "the last client out" is told from "one of several" instead).
const HOOKS: [(&str, &str); 3] = [
    (
        "client-session-changed",
        "\"#{session_name}\" \"#{client_last_session}\"",
    ),
    ("client-detached", "\"#{session_name}\""),
    // The backstop for a session torn down another way (`tmux kill-session`,
    // the shell in it exiting) while still attached.
    ("session-closed", "\"#{hook_session_name}\""),
];

/// Installs the global tmux hooks that drive `iter internal hook <event>
/// <session> [previous]`, for any event that doesn't already have a working
/// one. Safe to call every time a session is created.
///
/// "Already working" is deliberately not "already ours": a hook wired up in
/// `tmux.conf` is left exactly as it is. That's the difference between a
/// path baked in here -- `current_exe`, i.e. whichever build last ran
/// `session new`, which can be a debug binary in a worktree that later gets
/// deleted -- and a stable one the user chose. Installing over it every time
/// would quietly re-point the hooks at a binary that may not outlive the
/// afternoon.
pub fn ensure_hooks_installed(iter_bin: &str) -> Result<()> {
    for (event, args) in HOOKS {
        if hook_is_current(event, args) {
            continue;
        }
        let action = hook_action(iter_bin, event, args);
        Tmux.run(&["set-hook", "-g", event, &action])?;
    }
    Ok(())
}

/// Whether `event`'s installed hook already passes exactly the format
/// variables `args` calls for.
///
/// That comparison, rather than a string match on the whole action, is what
/// lets a `tmux.conf` hook -- different path, different quoting, same
/// variables -- count as current, while a hook from an older `iter` (which
/// passed `#{hook_session_name}` to every event, and so did nothing) does
/// not and gets replaced.
fn hook_is_current(event: &str, args: &str) -> bool {
    let Ok(existing) = Tmux.output(&["show-options", "-gv", event]) else {
        return false; // no tmux, or no such option: install ours
    };
    !existing.trim().is_empty() && format_variables(&existing) == format_variables(args)
}

/// The `#{...}` variables in a hook body, in order and without the quoting
/// around them, which differs between what we write and what a `tmux.conf`
/// does.
fn format_variables(action: &str) -> Vec<&str> {
    action
        .match_indices("#{")
        .filter_map(|(start, _)| {
            let rest = &action[start..];
            rest.find('}').map(|end| &rest[..=end])
        })
        .collect()
}

/// The `set-hook` body for one entry of [`HOOKS`]. Split out so the format
/// variables can be asserted without a tmux server, since naming the wrong
/// one is invisible at runtime -- the hook fires and quietly matches no
/// session.
fn hook_action(iter_bin: &str, event: &str, args: &str) -> String {
    format!("run-shell '{iter_bin} internal hook {event} {args}'")
}

/// Takes the detach message off `session`, if something left one there:
/// returns it and unsets the option, so it's consumed once rather than
/// re-attached to every later session of the same task.
///
/// Every failure is a `None`: the hook this serves fires for every tmux
/// session, most of which have no such option (and, on `session-closed`,
/// no longer exist to be asked).
pub fn take_detach_message(session: &str) -> Option<String> {
    let value =
        Tmux.output_trimmed(&["show-options", "-t", session, "-v", DETACH_MESSAGE_OPTION])?;
    let _ = Tmux.run(&["set-option", "-t", session, "-u", DETACH_MESSAGE_OPTION]);
    Some(value)
}

/// Creates the detached session named `name` in `cwd` and makes sure the
/// hooks that track it are installed -- the two steps `iter session new`
/// needs done together, and the part of it that can fail, which is why its
/// unwind path guards exactly this call.
pub fn start_tracked_session(name: &str, cwd: &str) -> Result<()> {
    create_session(name, cwd)?;
    let iter_bin = std::env::current_exe()?.to_string_lossy().to_string();
    ensure_hooks_installed(&iter_bin)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action_for(event: &str) -> String {
        let (event, args) = HOOKS
            .iter()
            .find(|(name, _)| *name == event)
            .unwrap_or_else(|| panic!("{event} is not an installed hook"));
        hook_action("/usr/bin/iter", event, args)
    }

    /// The bug this guards: every hook was installed with
    /// `#{hook_session_name}`, which tmux leaves empty on the client hooks
    /// -- so a detach named no session and silently closed nothing.
    #[test]
    fn the_client_hooks_are_told_the_session_by_session_name() {
        for event in ["client-session-changed", "client-detached"] {
            let action = action_for(event);
            assert!(
                action.contains("\"#{session_name}\""),
                "{event} must use #{{session_name}}: {action}"
            );
            assert!(
                !action.contains("hook_session_name"),
                "{event} is empty under #{{hook_session_name}}: {action}"
            );
        }
    }

    /// ...and the mirror image: on `session-closed` it's `#{session_name}`
    /// that's wrong, naming whatever session the client moved to rather
    /// than the one that closed.
    #[test]
    fn session_closed_is_told_the_session_by_hook_session_name() {
        let action = action_for("session-closed");
        assert!(action.contains("\"#{hook_session_name}\""), "{action}");
        assert!(!action.contains("\"#{session_name}\""), "{action}");
    }

    /// `client-session-changed` closes the session being left, so it needs
    /// that second argument -- without it a switch leaks an open session.
    #[test]
    fn client_session_changed_is_also_told_the_session_being_left() {
        let action = action_for("client-session-changed");
        assert!(action.contains("\"#{client_last_session}\""), "{action}");
    }

    /// A `tmux.conf` hook naming a stable path and quoting the variables
    /// its own way is still current -- that's what keeps `session new` from
    /// re-pointing it at whichever build happened to run.
    #[test]
    fn a_conf_written_hook_counts_as_current() {
        let (_, args) = HOOKS
            .iter()
            .find(|(name, _)| *name == "client-session-changed")
            .expect("the hook is installed");
        let from_conf = "run-shell \"~/bin/iter internal hook client-session-changed \
                         '#{session_name}' '#{client_last_session}'\"";
        assert_eq!(format_variables(from_conf), format_variables(args));
    }

    /// ...whereas the hook an older `iter` left behind does not, so it gets
    /// replaced rather than kept forever.
    #[test]
    fn a_stale_hook_from_an_older_iter_is_not_current() {
        let (_, args) = HOOKS
            .iter()
            .find(|(name, _)| *name == "client-detached")
            .expect("the hook is installed");
        let stale = "run-shell '/old/iter internal hook client-detached \"#{hook_session_name}\"'";
        assert_ne!(format_variables(stale), format_variables(args));
    }

    #[test]
    fn format_variables_reads_them_in_order_without_quoting() {
        assert_eq!(
            format_variables("a '#{session_name}' b \"#{client_last_session}\" c"),
            vec!["#{session_name}", "#{client_last_session}"]
        );
        assert_eq!(format_variables("run-shell 'echo hi'"), Vec::<&str>::new());
    }

    #[test]
    fn a_hook_action_invokes_the_internal_subcommand() {
        assert_eq!(
            action_for("client-detached"),
            "run-shell '/usr/bin/iter internal hook client-detached \"#{session_name}\"'"
        );
    }

    /// `#{client_name}` names a client that is still attached, not the one
    /// the hook fired for, so passing it would read as "somebody else is
    /// still here" on the very detach that should close the session.
    #[test]
    fn no_hook_passes_a_client_name() {
        for (event, _) in HOOKS {
            assert!(!action_for(event).contains("client_name"), "{event}");
        }
    }
}
