//! Running the external tools `iter` drives (`git`, `gh`, `tmux`, `nvim`).
//!
//! Every one of them fails the same two ways -- couldn't be spawned at all
//! (`IterError::Spawn`, carrying the `io::Error`) or ran and exited nonzero
//! (`IterError::CommandFailed`, which has no structured cause to carry, so
//! the message names the command instead). These helpers are where that
//! distinction is made, once, rather than at each of the call sites.

use crate::error::{IterError, Result};
use std::ffi::OsStr;
use std::process::Command;

fn command<S: AsRef<OsStr>>(tool: &'static str, dir: Option<&str>, args: &[S]) -> Command {
    let mut command = Command::new(tool);
    command.args(args);
    if let Some(dir) = dir {
        command.current_dir(dir);
    }
    command
}

/// Names a failed invocation, e.g. `git worktree add -b feat/x /tmp/x failed`.
fn failure_message<S: AsRef<OsStr>>(tool: &str, args: &[S]) -> String {
    let args: Vec<String> = args
        .iter()
        .map(|a| a.as_ref().to_string_lossy().into_owned())
        .collect();
    format!("{tool} {} failed", args.join(" "))
}

/// Runs `tool` to completion in `dir` (or the current directory), letting it
/// inherit stdio, and reports whether it exited zero. Only a failure to
/// *spawn* is an error here -- callers that read a nonzero exit as a plain
/// "no" rather than a failure (an editor quit without saving, say) use this
/// instead of [`run`].
pub fn run_status<S: AsRef<OsStr>>(
    tool: &'static str,
    dir: Option<&str>,
    args: &[S],
) -> Result<bool> {
    let status = command(tool, dir, args)
        .status()
        .map_err(|source| IterError::Spawn { tool, source })?;
    Ok(status.success())
}

/// Runs `tool` as [`run_status`] does, but treats a nonzero exit as an error.
pub fn run<S: AsRef<OsStr>>(tool: &'static str, dir: Option<&str>, args: &[S]) -> Result<()> {
    if run_status(tool, dir, args)? {
        Ok(())
    } else {
        Err(IterError::CommandFailed(failure_message(tool, args)))
    }
}

/// Runs `tool` and captures its stdout verbatim -- untrimmed, so callers
/// decide what whitespace is significant. A nonzero exit reports `stderr`
/// alongside the command, since a captured run prints nothing itself.
pub fn output<S: AsRef<OsStr>>(
    tool: &'static str,
    dir: Option<&str>,
    args: &[S],
) -> Result<String> {
    let output = command(tool, dir, args)
        .output()
        .map_err(|source| IterError::Spawn { tool, source })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr = stderr.trim();
        let message = failure_message(tool, args);
        return Err(IterError::CommandFailed(match stderr.is_empty() {
            true => message,
            false => format!("{message}: {stderr}"),
        }));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Whether `tool` ran *and* exited zero -- for the probe-style checks where
/// "couldn't run it" and "it said no" mean the same thing to the caller.
/// Suppresses stdio so the probe stays silent even when it fails.
pub fn succeeds<S: AsRef<OsStr>>(tool: &'static str, args: &[S]) -> bool {
    command(tool, None, args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}
