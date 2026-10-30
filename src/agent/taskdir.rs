//! A task's folder (`tasks/<id>/` inside its project's notes folder): its
//! plan, its own notes and the agent's sketches. It is kept with the notes,
//! not in the task's worktree, so discarding a branch does not discard what
//! was planned or learned.

use crate::db::{Db, Table};
use crate::error::{IterError, Result};
use crate::models::{Organization, Project, Task};
use crate::notes;
use std::path::{Path, PathBuf};
#[cfg(any(feature = "web", test))]
use std::time::SystemTime;

#[derive(Clone, Debug)]
pub struct TaskDir {
    root: PathBuf,
}

/// One markdown file of the folder.
#[cfg(any(feature = "web", test))]
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
    pub fn ensure(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(self.notes_dir())?;
        std::fs::create_dir_all(self.sketches_dir())
    }

    /// The plan's text, or `None` when there is no plan yet.
    pub fn read_plan(&self) -> std::io::Result<Option<String>> {
        match std::fs::read_to_string(self.plan_path()) {
            Ok(text) => Ok(Some(text)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Replaces the plan with `text`, all at once: a reader never sees half
    /// of it.
    pub fn write_plan(&self, text: &str) -> std::io::Result<()> {
        self.ensure()?;
        let tmp = self
            .root
            .join(format!(".plan.md.{}.tmp", std::process::id()));
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, self.plan_path())
    }

    /// The markdown files directly in `dir`, by name.
    #[cfg(any(feature = "web", test))]
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

/// The folder of `task`, which is in `project`. An error when neither the
/// project nor its organization says where notes are kept.
pub fn of(db: &Db, project: &Project, task: &Task) -> Result<TaskDir> {
    let org = match project.organization_id {
        Some(id) => db.get::<Organization>(id)?,
        None => None,
    };
    let dir = notes::project_dir(project, &org).ok_or_else(|| {
        IterError::CommandFailed(
            "no notes folder to keep the task in: set `notes_path` on the project or its organization"
                .into(),
        )
    })?;
    Ok(TaskDir::new(notes::task_dir(&dir, task.id())))
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
