//! The live version: one number that moves when what the pages show may have changed, so the web
//! app can update itself (`GET /live`). It moves when iter.db gets a commit from another
//! connection (SQLite's `data_version`, read on a connection kept open only for that) and when a
//! `.md` or `.yaml` file is made, changed or removed under an organization's or project's notes
//! folder or the config folder's `tasks/` (a task's notes, sketches, runs and plan). Anything
//! else leaves it alone. Asking costs a pragma, two small selects and a walk of those folders.

use crate::app::App;
use crate::config::config_dir;
use crate::db::Db;
use crate::error::{IterError, Result};
use crate::models::{Organization, Project};
use crate::notes;
use std::collections::BTreeSet;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};

/// How deep under a notes folder files are watched: `tasks/<id>/runs/<n>.yaml` is four down.
const DEPTH: usize = 6;

pub struct Version {
    /// A connection of its own: `data_version` ignores the connection's own commits, and this
    /// one never writes.
    db: Db,
}

impl Version {
    /// Watches the database `app` is over, and the folders its rows name. A database in memory
    /// has nothing another connection could commit to, so it is refused.
    pub fn open(app: &App) -> Result<Version> {
        let path = app.db.path().ok_or_else(|| {
            IterError::Io(std::io::Error::other(
                "a database in memory has no live version",
            ))
        })?;
        Ok(Version {
            db: Db::open(path)?,
        })
    }

    /// The version now: equal numbers mean nothing watched has changed in between.
    pub fn current(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        // an error reads as a version of its own, so a database that comes back moves it
        self.db.data_version().ok().hash(&mut hasher);
        for root in roots(&self.db) {
            let mut files = Vec::new();
            walk(&root, DEPTH, &mut files);
            files.sort();
            files.hash(&mut hasher);
        }
        hasher.finish()
    }
}

/// The folders to watch, a folder inside another watched one left out (a project's folder is
/// often inside its organization's).
fn roots(db: &Db) -> Vec<PathBuf> {
    let orgs: Vec<Organization> = db.list().unwrap_or_default();
    let projects: Vec<Project> = db.list().unwrap_or_default();
    let mut all: BTreeSet<PathBuf> = orgs.iter().filter_map(notes::org_dir).collect();
    for project in &projects {
        let org = project
            .organization_id
            .and_then(|id| orgs.iter().find(|o| o.id == Some(id)).cloned());
        all.extend(notes::project_dir(project, &org));
    }
    if let Ok(config) = config_dir() {
        all.insert(config.join("tasks"));
    }
    let mut roots: Vec<PathBuf> = Vec::new();
    // sorted, so a folder comes before the folders inside it
    for dir in all {
        if !roots.iter().any(|r| dir.starts_with(r)) {
            roots.push(dir);
        }
    }
    roots
}

/// What tells one state of a file from another.
#[derive(Hash, PartialEq, Eq, PartialOrd, Ord)]
struct Seen {
    path: PathBuf,
    len: u64,
    modified: Option<std::time::SystemTime>,
}

fn watched(path: &Path) -> bool {
    let hidden = path
        .file_name()
        .and_then(|n| n.to_str())
        .is_none_or(|n| n.starts_with('.'));
    let ext = path.extension().and_then(|x| x.to_str());
    !hidden && matches!(ext, Some("md" | "yaml" | "yml"))
}

/// The watched files under `dir`, hidden folders (`.git`, `.obsidian`) and links skipped.
fn walk(dir: &Path, depth: usize, out: &mut Vec<Seen>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if kind.is_dir() {
            let hidden = entry.file_name().to_string_lossy().starts_with('.');
            if depth > 0 && !hidden {
                walk(&path, depth - 1, out);
            }
        } else if kind.is_file()
            && watched(&path)
            && let Ok(meta) = entry.metadata()
        {
            out.push(Seen {
                path,
                len: meta.len(),
                modified: meta.modified().ok(),
            });
        }
    }
}
