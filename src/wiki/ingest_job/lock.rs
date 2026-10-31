//! The wiki lock: one job at a time writes a project's wiki.

use super::*;

/// The lock of a running job: its file, if a process holds it.
pub fn lock_path(target: &Target) -> PathBuf {
    target.notes_dir.join(".iter").join("wiki.lock")
}

pub fn busy(target: &Target) -> bool {
    std::fs::read_to_string(lock_path(target))
        .ok()
        .and_then(|t| t.trim().parse::<u32>().ok())
        .is_some_and(alive)
}

/// The lock of a job, held until it is dropped.
pub struct Lock(PathBuf);

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Takes the lock, or says who has it: the file is made with a call that
/// fails if it exists, and one left by a process that is gone is replaced.
pub fn lock(target: &Target) -> Result<Lock> {
    let path = lock_path(target);
    std::fs::create_dir_all(path.parent().unwrap_or(&target.notes_dir))?;
    for _ in 0..2 {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                use std::io::Write;
                write!(file, "{}", std::process::id())?;
                return Ok(Lock(path));
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                if busy(target) {
                    return Err(AgentError::WikiBusy.into());
                }
                let _ = std::fs::remove_file(&path);
            }
            Err(e) => return Err(e.into()),
        }
    }
    Err(AgentError::WikiLockUnavailable.into())
}
