use crate::error::Result;
use crate::process;

/// Creates a new detached tmux session named `name`, starting in `cwd`.
pub fn create_session(name: &str, cwd: &str) -> Result<()> {
    process::run("tmux", None, &["new-session", "-d", "-s", name, "-c", cwd])
}

/// Kills a tmux session if it's running. Not an error if it's already gone.
pub fn kill_session(name: &str) {
    let _ = process::run("tmux", None, &["kill-session", "-t", name]);
}

pub fn session_exists(name: &str) -> bool {
    process::succeeds("tmux", &["has-session", "-t", name])
}

/// The tmux session name of the pane this process is running in, if any
/// (i.e. we're inside a tmux client). Used by `iter t`.
pub fn current_session_name() -> Option<String> {
    let name = process::output("tmux", None, &["display-message", "-p", "#S"]).ok()?;
    let name = name.trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

/// The three hook events `iter` cares about. `client-attached` starts a
/// record, `client-detached` ends it (this fires even when the client goes
/// away uncleanly -- e.g. the terminal emulator is killed -- since tmux
/// notices the client's socket disappear regardless of cause), and
/// `session-closed` is the backstop for a session being torn down another
/// way (e.g. `tmux kill-session`) while still attached.
const HOOK_EVENTS: [&str; 3] = ["client-attached", "client-detached", "session-closed"];

/// Idempotently installs the global tmux hooks that drive `iter internal
/// hook <event> <session>`. Safe to call every time a session is created.
pub fn ensure_hooks_installed(iter_bin: &str) -> Result<()> {
    for event in HOOK_EVENTS {
        // `#{hook_session_name}` is populated for every hook, giving us the
        // session the event fired on without needing per-session hook wiring.
        let action =
            format!("run-shell '{iter_bin} internal hook {event} \"#{{hook_session_name}}\"'");
        process::run("tmux", None, &["set-hook", "-g", event, &action])?;
    }
    Ok(())
}
