//! Starting agent processes: waited for with a log, or detached.

use crate::error::{IterError, Result};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};

/// Runs `program` in `cwd` to the end, output appended to `log`, stdin closed.
pub fn run_logged(
    program: &'static str,
    args: &[String],
    cwd: &Path,
    log: std::fs::File,
) -> Result<ExitStatus> {
    Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .status()
        .map_err(|source| IterError::Spawn {
            tool: program,
            source,
        })
}

/// The exit code a run records: `0` on success, `-1` when a signal ended it.
pub fn exit_code(status: ExitStatus) -> Option<i32> {
    status.code().or(Some(-1))
}

/// Starts `command` (iter itself) in its own process group, so a signal to iter's does not reach it; returns the pid.
/// A thread waits for the child so it does not linger as a zombie in a long-lived process.
pub fn spawn_detached(command: &mut Command) -> Result<u32> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map_err(|source| IterError::Spawn {
            tool: "iter",
            source,
        })?;
    let pid = child.id();
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(pid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_logged_run_writes_its_output_and_reports_its_code() {
        let dir = crate::fixtures::scratch("exec");
        std::fs::create_dir_all(&dir).unwrap();
        let log_path = dir.join("log");
        let log = std::fs::File::create(&log_path).unwrap();
        let status = run_logged("sh", &["-c".into(), "echo hi; exit 3".into()], &dir, log).unwrap();
        assert_eq!(exit_code(status), Some(3));
        assert_eq!(std::fs::read_to_string(&log_path).unwrap(), "hi\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_program_is_a_spawn_error() {
        let log = std::fs::File::create(std::env::temp_dir().join("iter-exec-missing")).unwrap();
        let err = run_logged("iter-no-such-program", &[], Path::new("."), log).unwrap_err();
        assert!(matches!(err, IterError::Spawn { .. }));
    }
}
