//! Running external tools (`git`, `gh`, `tmux`, `nvim`). A failure is either unspawnable
//! (`IterError::Spawn`) or a nonzero exit (`IterError::CommandFailed`, whose message names the
//! command); these helpers draw that distinction once.

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

/// Runs `tool` with inherited stdio and reports whether it exited zero. Only a spawn failure is
/// an error: for callers where nonzero is a plain "no" (an editor quit without saving).
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

/// Captures stdout verbatim (untrimmed). A nonzero exit reports `stderr` with the command.
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
    // Moves the buffer rather than copying: `gh issue list` output can run to megabytes.
    Ok(String::from_utf8(output.stdout)
        .unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned()))
}

/// [`output`] trimmed; every failure, including empty output, is `None`.
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

/// Whether `tool` ran and exited zero, silently. For probes where "couldn't run" and "said no"
/// mean the same.
pub fn succeeds<S: AsRef<OsStr>>(tool: &'static str, dir: Option<&str>, args: &[S]) -> bool {
    command(tool, dir, args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// An external binary `iter` drives, bound to its run directory. It only saves call sites from
/// repeating the tool name and `dir`; the free functions above hold the failure policies. Not
/// object-safe on purpose.
pub trait Tool {
    const BIN: &'static str;

    /// The directory to run in; `None` is the current one.
    fn dir(&self) -> Option<&str> {
        None
    }

    fn run<S: AsRef<OsStr>>(&self, args: &[S]) -> Result<()> {
        run(Self::BIN, self.dir(), args)
    }

    /// [`run_status`] for a tool: a nonzero exit is an answer, not a failure (a merge stopping on a
    /// conflict, whose output is the report).
    fn run_status<S: AsRef<OsStr>>(&self, args: &[S]) -> Result<bool> {
        run_status(Self::BIN, self.dir(), args)
    }

    fn output_trimmed<S: AsRef<OsStr>>(&self, args: &[S]) -> Option<String> {
        output_trimmed(Self::BIN, self.dir(), args)
    }
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

/// The command runner behind the tool clients (`GhCli`, `TmuxClient`): object-safe, so a fake can
/// record argv instead of spawning.
pub trait Runner {
    /// See [`run_status`].
    fn run_status(&self, tool: &'static str, dir: Option<&str>, args: &[&str]) -> Result<bool>;
    /// See [`output`].
    fn output(&self, tool: &'static str, dir: Option<&str>, args: &[&str]) -> Result<String>;
    /// See [`succeeds`].
    fn succeeds(&self, tool: &'static str, dir: Option<&str>, args: &[&str]) -> bool;

    /// See [`run`].
    fn run(&self, tool: &'static str, dir: Option<&str>, args: &[&str]) -> Result<()> {
        match self.run_status(tool, dir, args)? {
            true => Ok(()),
            false => Err(IterError::CommandFailed(failure_message(tool, args))),
        }
    }

    /// See [`output_trimmed`].
    fn output_trimmed(
        &self,
        tool: &'static str,
        dir: Option<&str>,
        args: &[&str],
    ) -> Option<String> {
        let value = self.output(tool, dir, args).ok()?;
        let value = value.trim();
        match value.is_empty() {
            true => None,
            false => Some(value.to_string()),
        }
    }
}

pub struct SystemRunner;

impl Runner for SystemRunner {
    fn run_status(&self, tool: &'static str, dir: Option<&str>, args: &[&str]) -> Result<bool> {
        run_status(tool, dir, args)
    }
    fn output(&self, tool: &'static str, dir: Option<&str>, args: &[&str]) -> Result<String> {
        output(tool, dir, args)
    }
    fn succeeds(&self, tool: &'static str, dir: Option<&str>, args: &[&str]) -> bool {
        succeeds(tool, dir, args)
    }
}

pub static SYSTEM: SystemRunner = SystemRunner;

/// A [`Runner`] that records every invocation as `tool arg arg...` and
/// answers from a script, for tests.
#[cfg(test)]
pub mod fake {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    pub struct FakeRunner {
        pub calls: RefCell<Vec<String>>,
        /// `(prefix, stdout)`: the first entry whose prefix starts a call's
        /// `tool args` line answers it with that stdout.
        pub replies: RefCell<Vec<(String, String)>>,
        /// Prefixes of calls that exit nonzero.
        pub failing: RefCell<Vec<String>>,
    }

    impl FakeRunner {
        pub fn reply(&self, prefix: &str, stdout: &str) {
            self.replies
                .borrow_mut()
                .push((prefix.into(), stdout.into()));
        }
        pub fn fail(&self, prefix: &str) {
            self.failing.borrow_mut().push(prefix.into());
        }
        pub fn calls(&self) -> Vec<String> {
            self.calls.borrow().clone()
        }
        fn record(&self, tool: &str, dir: Option<&str>, args: &[&str]) -> (String, bool) {
            let line = format!("{tool} {}", args.join(" "));
            let line = line.trim_end().to_string();
            let ok = !self.failing.borrow().iter().any(|p| line.starts_with(p));
            let shown = match dir {
                Some(dir) => format!("[{dir}] {line}"),
                None => line.clone(),
            };
            self.calls.borrow_mut().push(shown);
            (line, ok)
        }
    }

    impl Runner for FakeRunner {
        fn run_status(&self, tool: &'static str, dir: Option<&str>, args: &[&str]) -> Result<bool> {
            Ok(self.record(tool, dir, args).1)
        }
        fn output(&self, tool: &'static str, dir: Option<&str>, args: &[&str]) -> Result<String> {
            let (line, ok) = self.record(tool, dir, args);
            if !ok {
                return Err(IterError::CommandFailed(failure_message(tool, args)));
            }
            let replies = self.replies.borrow();
            Ok(replies
                .iter()
                .find(|(p, _)| line.starts_with(p))
                .map(|(_, out)| out.clone())
                .unwrap_or_default())
        }
        fn succeeds(&self, tool: &'static str, dir: Option<&str>, args: &[&str]) -> bool {
            self.record(tool, dir, args).1
        }
    }
}
