use crate::error::{IterError, Result};
use std::process::Command;

/// Creates a new detached tmux session named `name`, starting in `cwd`.
pub fn create_session(name: &str, cwd: &str) -> Result<()> {
    let status = Command::new("tmux")
        .args(["new-session", "-d", "-s", name, "-c", cwd])
        .status()
        .map_err(|source| IterError::Spawn {
            tool: "tmux",
            source,
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(IterError::CommandFailed(format!(
            "tmux new-session -s {name} failed"
        )))
    }
}

/// Kills a tmux session if it's running. Not an error if it's already gone.
pub fn kill_session(name: &str) {
    let _ = Command::new("tmux")
        .args(["kill-session", "-t", name])
        .status();
}

pub fn session_exists(name: &str) -> bool {
    Command::new("tmux")
        .args(["has-session", "-t", name])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// The tmux session name of the pane this process is running in, if any
/// (i.e. we're inside a tmux client). Used by `iter t`.
pub fn current_session_name() -> Option<String> {
    let output = Command::new("tmux")
        .args(["display-message", "-p", "#S"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if name.is_empty() { None } else { Some(name) }
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
        install_hook(event, iter_bin)?;
    }
    Ok(())
}

fn install_hook(event: &str, iter_bin: &str) -> Result<()> {
    // `#{hook_session_name}` is populated for every hook, giving us the
    // session the event fired on without needing per-session hook wiring.
    let action = format!("run-shell '{iter_bin} internal hook {event} \"#{{hook_session_name}}\"'");
    let status = Command::new("tmux")
        .args(["set-hook", "-g", event, &action])
        .status()
        .map_err(|source| IterError::Spawn {
            tool: "tmux",
            source,
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(IterError::CommandFailed(format!(
            "failed to install the {event} tmux hook"
        )))
    }
}
