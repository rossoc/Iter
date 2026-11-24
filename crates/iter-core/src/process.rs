//! Running external tools (`git`, `gh`, `tmux`, `nvim`) and quoting for the shell. A failure is
//! either unspawnable (`IterError::Spawn`) or a nonzero exit (`IterError::CommandFailed`, whose
//! message names the command); the free functions draw that distinction once, [`Runner`] lets a
//! fake stand in for them, and [`Tool`] binds a binary to its directory.

use crate::error::{IterError, Result};
use std::ffi::OsStr;
use std::process::{Command, Stdio};

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
    let args: Vec<_> = args.iter().map(|a| a.as_ref().to_string_lossy()).collect();
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
        let message = failure_message(tool, args);
        return Err(IterError::CommandFailed(match stderr.trim() {
            "" => message,
            stderr => format!("{message}: {stderr}"),
        }));
    }
    // Moves the buffer rather than copying: `gh issue list` output can run to megabytes.
    Ok(String::from_utf8(output.stdout)
        .unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned()))
}

/// Whether `tool` ran and exited zero, silently. For probes where "couldn't run" and "said no"
/// mean the same.
pub fn succeeds<S: AsRef<OsStr>>(tool: &'static str, dir: Option<&str>, args: &[S]) -> bool {
    command(tool, dir, args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// An external binary `iter` drives, bound to its run directory: it saves call sites from
/// repeating the tool name and `dir`, and runs through [`SYSTEM`].
pub trait Tool {
    const BIN: &'static str;

    /// The directory to run in; `None` is the current one.
    fn dir(&self) -> Option<&str> {
        None
    }

    fn run(&self, args: &[&str]) -> Result<()> {
        SYSTEM.run(Self::BIN, self.dir(), args)
    }

    /// [`Runner::run_status`] for a tool: a nonzero exit is an answer, not a failure (a merge
    /// stopping on a conflict, whose output is the report).
    fn run_status(&self, args: &[&str]) -> Result<bool> {
        SYSTEM.run_status(Self::BIN, self.dir(), args)
    }

    fn output_trimmed(&self, args: &[&str]) -> Option<String> {
        SYSTEM.output_trimmed(Self::BIN, self.dir(), args)
    }
}

/// `git`, in the repository at the given path -- or, for `clone`, wherever the process already is.
pub struct Git<'a>(pub Option<&'a str>);

impl Tool for Git<'_> {
    const BIN: &'static str = "git";

    fn dir(&self) -> Option<&str> {
        self.0
    }
}

/// The command runner behind the tool clients (`GhCli`, `TmuxClient`) and [`Tool`]: object-safe,
/// so a fake can record argv instead of spawning.
pub trait Runner {
    /// See [`run_status`].
    fn run_status(&self, tool: &'static str, dir: Option<&str>, args: &[&str]) -> Result<bool>;
    /// See [`output`].
    fn output(&self, tool: &'static str, dir: Option<&str>, args: &[&str]) -> Result<String>;
    /// See [`succeeds`].
    fn succeeds(&self, tool: &'static str, dir: Option<&str>, args: &[&str]) -> bool;

    /// A nonzero exit is an error naming the command.
    fn run(&self, tool: &'static str, dir: Option<&str>, args: &[&str]) -> Result<()> {
        match self.run_status(tool, dir, args)? {
            true => Ok(()),
            false => Err(IterError::CommandFailed(failure_message(tool, args))),
        }
    }

    /// [`Runner::output`] trimmed; every failure, including empty output, is `None`.
    fn output_trimmed(
        &self,
        tool: &'static str,
        dir: Option<&str>,
        args: &[&str],
    ) -> Option<String> {
        let output = self.output(tool, dir, args).ok()?;
        Some(output.trim().to_string()).filter(|value| !value.is_empty())
    }
}

/// Spawns the real processes, through the free functions above.
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
#[cfg(any(test, feature = "test-support"))]
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
            // the space an empty last argument leaves goes; a newline ending an argument (a
            // pasted prompt) is the argument's own and stays
            let line = line.trim_end_matches(' ').to_string();
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

/// The `iter` binary that is running: the one place the core asks for it, to start `iter` again
/// as a detached worker or to name it in a tmux hook.
pub fn host() -> Result<std::path::PathBuf> {
    Ok(std::env::current_exe()?)
}

/// `word` as one shell word, backslash-escaped (`app/Fix\ it`): a newline is quoted (it cannot
/// be escaped), an empty word is `''`. The tmux hook and the prompts both need it.
pub fn shell_word(word: &str) -> String {
    if word.is_empty() {
        return "''".to_string();
    }
    let plain = |c: char| c.is_ascii_alphanumeric() || "_./:-@%+=,".contains(c);
    word.chars().fold(String::new(), |mut out, c| {
        match c {
            '\n' => out.push_str("'\n'"),
            c if plain(c) || !c.is_ascii() => out.push(c),
            c => out.extend(['\\', c]),
        }
        out
    })
}

#[cfg(test)]
mod host_and_shell_tests {
    use super::*;

    #[test]
    fn the_host_is_the_running_binary() {
        assert_eq!(host().unwrap(), std::env::current_exe().unwrap());
    }

    /// Each case reads back as one word in `sh`: that is the property, the spelling is the pin.
    #[test]
    fn a_shell_word_is_escaped_so_a_shell_reads_it_as_one_word() {
        for (word, quoted) in [
            ("", "''"),
            ("app/fix-it", "app/fix-it"),
            ("app/Fix it", r"app/Fix\ it"),
            ("app/It's", r"app/It\'s"),
            ("a;b", r"a\;b"),
            ("$HOME", r"\$HOME"),
            ("a\"b", "a\\\"b"),
            ("x&y|z", r"x\&y\|z"),
            ("42", "42"),
            ("user@host:1+2=3,4%5", "user@host:1+2=3,4%5"),
            ("naïve ✓", r"naïve\ ✓"),
            ("two\nlines", "two'\n'lines"),
            ("-dash", "-dash"),
            ("a;rm -rf", r"a\;rm\ -rf"),
            ("café au", r"café\ au"),
        ] {
            assert_eq!(shell_word(word), quoted, "{word:?}");
        }
    }
}
