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
    // Moves the buffer rather than copying it: `gh issue list` output can
    // run to megabytes, and it is valid UTF-8 in every case but a broken
    // one, which still falls back rather than failing.
    Ok(String::from_utf8(output.stdout)
        .unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned()))
}

/// [`output`] trimmed, with every failure -- couldn't spawn, nonzero exit,
/// nothing but whitespace printed -- collapsed into `None`. What the
/// probe-style readers want, which would otherwise each re-spell the same
/// `.ok()?` / trim / is-empty dance.
pub fn output_trimmed<S: AsRef<OsStr>>(
    tool: &'static str,
    dir: Option<&str>,
    args: &[S],
) -> Option<String> {
    let value = output(tool, dir, args).ok()?;
    let value = value.trim();
    match value.is_empty() {
        true => None,
        false => Some(value.to_string()),
    }
}

/// Whether `tool` ran *and* exited zero -- for the probe-style checks where
/// "couldn't run it" and "it said no" mean the same thing to the caller.
/// Suppresses stdio so the probe stays silent even when it fails.
pub fn succeeds<S: AsRef<OsStr>>(tool: &'static str, dir: Option<&str>, args: &[S]) -> bool {
    command(tool, dir, args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// An external binary `iter` drives, bound to the directory it runs in.
///
/// The five free functions above are the mechanism -- each encodes a
/// different failure policy, and that distinction is the point of this
/// module. This trait adds nothing to it; it only stops every call site
/// from repeating the tool's name and threading a `dir` that, for some
/// tools, can never be anything but `None`.
///
/// Not object-safe, and deliberately so: the methods stay generic over the
/// argument type, and nothing here needs `dyn`.
pub trait Tool {
    /// The binary's name, as it is looked up on `PATH`.
    const BIN: &'static str;

    /// The directory to run in. `None` -- the default -- is the current
    /// one, which is right for a tool that takes its target as an argument
    /// rather than inferring it from the working directory.
    fn dir(&self) -> Option<&str> {
        None
    }

    fn run<S: AsRef<OsStr>>(&self, args: &[S]) -> Result<()> {
        run(Self::BIN, self.dir(), args)
    }

    /// [`run_status`] for a tool: the exit status as a plain yes/no, with
    /// the tool's own output left on the terminal. For the callers that
    /// read a nonzero exit as an answer rather than a failure -- a merge
    /// that stops on a conflict, say, whose output *is* the report.
    fn run_status<S: AsRef<OsStr>>(&self, args: &[S]) -> Result<bool> {
        run_status(Self::BIN, self.dir(), args)
    }

    fn output<S: AsRef<OsStr>>(&self, args: &[S]) -> Result<String> {
        output(Self::BIN, self.dir(), args)
    }

    fn output_trimmed<S: AsRef<OsStr>>(&self, args: &[S]) -> Option<String> {
        output_trimmed(Self::BIN, self.dir(), args)
    }

    fn succeeds<S: AsRef<OsStr>>(&self, args: &[S]) -> bool {
        succeeds(Self::BIN, self.dir(), args)
    }
}

/// `tmux`, which is always addressed by session name rather than by
/// directory -- hence no `dir`.
pub struct Tmux;

impl Tool for Tmux {
    const BIN: &'static str = "tmux";
}

/// `git`, in the repository at the given path -- or, for `clone`, wherever
/// the process already is.
pub struct Git<'a>(pub Option<&'a str>);

impl Tool for Git<'_> {
    const BIN: &'static str = "git";

    fn dir(&self) -> Option<&str> {
        self.0
    }
}

/// `gh`, in a repository whose remote tells it which GitHub repo is meant.
/// `iter` never stores or parses an "owner/repo" string of its own, so the
/// directory is the whole of the addressing.
pub struct Gh<'a>(pub &'a str);

impl Tool for Gh<'_> {
    const BIN: &'static str = "gh";

    fn dir(&self) -> Option<&str> {
        Some(self.0)
    }
}
