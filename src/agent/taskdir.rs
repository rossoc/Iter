//! A task's folder (`tasks/<id>/` inside its project's notes folder): its
//! plan, its own notes and the agent's sketches. It is kept with the notes,
//! not in the task's worktree, so discarding a branch does not discard what
//! was planned or learned.

use crate::agent::error::AgentResult;
use crate::db::{Db, Table};
use crate::error::{IterError, Result};
use crate::models::{Organization, Project, Task};
use crate::notes;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Clone, Debug)]
pub struct TaskDir {
    root: PathBuf,
}

/// One markdown file of the folder.
#[derive(Clone, Debug, PartialEq)]
pub struct File {
    /// The name without `.md`.
    pub name: String,
    pub path: PathBuf,
    pub modified: Option<SystemTime>,
}

impl TaskDir {
    pub fn new(root: PathBuf) -> TaskDir {
        TaskDir { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn plan_path(&self) -> PathBuf {
        self.root.join("plan.md")
    }

    /// The plan before the last one `iter task plan` replaced.
    pub fn previous_plan_path(&self) -> PathBuf {
        self.root.join("plan.prev.md")
    }

    pub fn notes_dir(&self) -> PathBuf {
        self.root.join("notes")
    }

    pub fn sketches_dir(&self) -> PathBuf {
        self.root.join("sketches")
    }

    /// Makes the folder and its two subfolders if they are not there.
    pub fn ensure(&self) -> AgentResult<()> {
        std::fs::create_dir_all(self.notes_dir())?;
        Ok(std::fs::create_dir_all(self.sketches_dir())?)
    }

    /// The plan's text, or `None` when there is no plan yet.
    pub fn read_plan(&self) -> AgentResult<Option<String>> {
        match std::fs::read_to_string(self.plan_path()) {
            Ok(text) => Ok(Some(text)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Replaces the plan with `text`, all at once: a reader never sees half
    /// of it.
    pub fn write_plan(&self, text: &str) -> AgentResult<()> {
        self.ensure()?;
        Ok(write_whole(&self.root, &self.plan_path(), text)?)
    }

    /// Reads the plan, lets `change` make the new one from it, and writes
    /// that, all while holding the plan's lock: two writers (an agent ticking
    /// a step, a click on the page) take turns instead of one losing its
    /// change. `change` returns `None` to leave the plan as it is.
    pub fn update_plan<T>(
        &self,
        change: impl FnOnce(Option<String>) -> std::result::Result<(Option<String>, T), IterError>,
    ) -> Result<T> {
        self.ensure()?;
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(self.root.join(".plan.lock"))?;
        lock.lock()?;
        let (new, out) = change(self.read_plan()?)?;
        if let Some(text) = new {
            self.write_plan(&text)?;
        }
        Ok(out)
        // the lock is released when `lock` is dropped
    }

    /// The markdown files directly in `dir`, by name.
    pub fn files(dir: &Path) -> Vec<File> {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut files: Vec<File> = entries
            .flatten()
            .filter_map(|e| {
                let path = e.path();
                let name = path.file_stem()?.to_str()?.to_string();
                let md = path.extension().is_some_and(|x| x == "md");
                (md && path.is_file() && !name.starts_with('.')).then(|| File {
                    name,
                    modified: e.metadata().and_then(|m| m.modified()).ok(),
                    path,
                })
            })
            .collect();
        files.sort_by(|a, b| a.name.cmp(&b.name));
        files
    }
}

/// Writes `text` to `target` through a file of its own beside it, then renames
/// it over: the name is unique to this write, so writers in one process do not
/// share it either.
pub(crate) fn write_whole(dir: &Path, target: &Path, text: &str) -> std::io::Result<()> {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = dir.join(format!(".write.{}.{n}.tmp", std::process::id()));
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, target).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// [`write_whole`] for a `target` that is wherever it is: the file is either
/// the old text or the new one, never a half-written one that a crash, or a
/// reader at that moment, would see.
pub(crate) fn write_atomic(target: &Path, text: &str) -> std::io::Result<()> {
    let dir = target.parent().filter(|d| !d.as_os_str().is_empty());
    write_whole(dir.unwrap_or(Path::new(".")), target, text)
}

/// The folder of `task`, which is in `project`: inside the project's notes
/// folder, or -- when neither the project nor its organization has one -- in
/// iter's own config directory (`tasks/<id>/`), so an agent can still work on
/// the task. Moving to a notes folder later leaves that one behind.
pub fn of(db: &Db, project: &Project, task: &Task) -> Result<TaskDir> {
    let org = match project.organization_id {
        Some(id) => db.get::<Organization>(id)?,
        None => None,
    };
    Ok(TaskDir::new(match notes::project_dir(project, &org) {
        Some(dir) => notes::task_dir(&dir, task.id()),
        None => notes::task_dir(&crate::config::config_dir()?, task.id()),
    }))
}

/// Whether the folder of `task` is among the project's notes (and so part of
/// its wiki) rather than the fallback in the config directory.
pub fn in_notes(db: &Db, project: &Project) -> bool {
    let org = project
        .organization_id
        .and_then(|id| db.get::<Organization>(id).ok().flatten());
    notes::project_dir(project, &org).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("iter-taskdir-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_plan_is_written_whole_and_read_back() {
        let dir = TaskDir::new(scratch("plan").join("tasks/7"));
        assert_eq!(dir.read_plan().unwrap(), None);
        dir.write_plan("## Goal\nx\n").unwrap();
        assert_eq!(dir.read_plan().unwrap().as_deref(), Some("## Goal\nx\n"));
        dir.write_plan("## Goal\ny\n").unwrap();
        assert_eq!(dir.read_plan().unwrap().as_deref(), Some("## Goal\ny\n"));
        // the folder has its two subfolders, and no temporary file is left
        assert!(dir.notes_dir().is_dir() && dir.sketches_dir().is_dir());
        let left: Vec<_> = std::fs::read_dir(dir.root())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert!(left.iter().all(|n| !n.ends_with(".tmp")), "{left:?}");
    }

    #[test]
    fn files_are_the_markdown_ones_by_name() {
        let dir = TaskDir::new(scratch("files"));
        dir.ensure().unwrap();
        for name in ["b.md", "a.md", "c.txt", ".hidden.md"] {
            std::fs::write(dir.sketches_dir().join(name), "x").unwrap();
        }
        let names: Vec<_> = TaskDir::files(&dir.sketches_dir())
            .into_iter()
            .map(|f| f.name)
            .collect();
        assert_eq!(names, ["a", "b"]);
        assert!(TaskDir::files(&dir.root().join("missing")).is_empty());
    }
}
