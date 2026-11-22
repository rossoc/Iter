//! Addresses of what `iter note` reads and writes, mirroring the notes folders, so nobody (an agent
//! least of all) has to know a path:
//!
//! - `[org/][project/]note[/<YYYY-MM-DD>]` or `[org/][project/]<YYYY-MM-DD>`: a daily note (today);
//! - `[org/][project/]wiki/<class>/<name>`: a wiki page (`org/wiki/...` is the organization's);
//! - `[org/][project/]tasks/<task>/sketch/<name>`, `.../note/<name>`: a task's sketch or note;
//! - `sketch/<name>`, `note/<name>`: the current task's.
//!
//! What is left out comes from where iter runs (`locate::current_task`/`current_project`).
//! A wiki class's folder is the ontology's (`iter:place`), so a project's extension adds classes.

use crate::agent::taskdir;
use crate::db::Db;
use crate::error::{IterError, Result};
use crate::locate::{self, current_project, current_task};
use crate::models::{Organization, Project, Task};
use crate::notes;
use crate::wiki::ontology::{LevelKind, NS, Ontology};
use chrono::NaiveDate;
use std::path::PathBuf;

/// What kind of document an address names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Doc {
    /// A daily note; `None` is today's.
    Daily(Option<NaiveDate>),
    /// A file in a task's `notes/` (the user's).
    TaskNote,
    /// A file in a task's `sketches/` (the agent's).
    Sketch,
    /// A wiki page of this class (`concept`, `concepts`, `Concept` alike).
    Page(String),
}

/// A parsed address: the names given (empty ones come from where iter runs) and the document.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Address {
    /// What came before the document: `[]`, `[project]` (or `[org]`), `[org, project]`.
    pub owners: Vec<String>,
    /// `tasks/<task>`: an id, or a name in the project.
    pub task: Option<String>,
    pub doc: Option<Doc>,
    pub name: Option<String>,
}

impl Address {
    /// An address for `doc` named `name` here, or in the project or organization given.
    pub fn of(doc: Doc, name: Option<&str>, project: Option<&str>, org: Option<&str>) -> Address {
        Address {
            owners: org.into_iter().chain(project).map(String::from).collect(),
            task: None,
            doc: Some(doc),
            name: name.map(String::from),
        }
    }
}

fn bad(text: &str, why: &str) -> IterError {
    IterError::CommandFailed(format!("'{text}' is not a note address: {why}"))
}

fn is_sketch(word: &str) -> bool {
    matches!(word, "sketch" | "sketches")
}

fn is_note(word: &str) -> bool {
    matches!(word, "note" | "notes")
}

/// Reads `text` (see the module doc). Checks only the shape; names are looked up by [`locate`].
pub fn parse(text: &str) -> Result<Address> {
    let trimmed = text.trim().trim_matches('/');
    let trimmed = trimmed.strip_suffix(".md").unwrap_or(trimmed);
    let parts: Vec<&str> = trimmed.split('/').collect();
    if parts
        .iter()
        .any(|p| p.is_empty() || p.starts_with('.') || p.contains('\\'))
    {
        return Err(bad(text, "empty, hidden or `..` parts are not allowed"));
    }
    let start = parts
        .iter()
        .position(|p| {
            is_note(p)
                || is_sketch(p)
                || *p == "wiki"
                || *p == "tasks"
                || notes::parse_date(p).is_some()
        })
        .ok_or_else(|| bad(text, "say note, wiki, tasks or sketch"))?;
    let owners: Vec<String> = parts[..start].iter().map(|p| p.to_string()).collect();
    if owners.len() > 2 {
        return Err(bad(
            text,
            "at most an organization and a project come first",
        ));
    }
    let name = |p: &str| Some(p.to_string());
    let (task, doc, name) = match &parts[start..] {
        [day] if notes::parse_date(day).is_some() => {
            (None, Doc::Daily(notes::parse_date(day)), None)
        }
        [n] if is_note(n) => (None, Doc::Daily(None), None),
        [n, day] if is_note(n) && notes::parse_date(day).is_some() => {
            (None, Doc::Daily(notes::parse_date(day)), None)
        }
        [n, file] if is_note(n) => (None, Doc::TaskNote, name(file)),
        [s, file] if is_sketch(s) => (None, Doc::Sketch, name(file)),
        ["wiki", class, file] => (None, Doc::Page(class.to_string()), name(file)),
        ["tasks", task, kind, file] if is_note(kind) || is_sketch(kind) => {
            let doc = if is_note(kind) {
                Doc::TaskNote
            } else {
                Doc::Sketch
            };
            (Some(task.to_string()), doc, name(file))
        }
        _ => return Err(bad(text, "see `iter note read --help` for the shapes")),
    };
    Ok(Address {
        owners,
        task,
        doc: Some(doc),
        name,
    })
}

/// Where an address leads: the folder and, for a named document, its file.
#[derive(Debug)]
pub struct Located {
    pub dir: PathBuf,
    pub file: Option<PathBuf>,
}

fn no_notes() -> IterError {
    IterError::CommandFailed("no notes folder: set `notes_path` on it or its organization".into())
}

/// The level a daily note or a wiki page belongs to: a project, or an organization.
enum Level {
    Project(Project, Option<Organization>),
    Organization(Organization),
}

impl Level {
    fn dir(&self) -> Result<PathBuf> {
        match self {
            Level::Project(p, org) => notes::project_dir(p, org),
            Level::Organization(org) => notes::org_dir(org),
        }
        .ok_or_else(no_notes)
    }

    fn kind(&self) -> LevelKind {
        match self {
            Level::Project(..) => LevelKind::Project,
            Level::Organization(_) => LevelKind::Organization,
        }
    }
}

/// A project by name, held to belonging to `org` when one was named.
fn project_in(db: &Db, name: &str, org: Option<&str>) -> Result<Project> {
    let project = db.resolve::<Project>(name)?;
    if let Some(org) = org {
        let belongs = db.org_of(&project)?.is_some_and(|o| o.name == org);
        if !belongs {
            return Err(IterError::CommandFailed(format!(
                "project '{name}' is not in organization '{org}'"
            )));
        }
    }
    Ok(project)
}

fn level(db: &Db, owners: &[String]) -> Result<Level> {
    let with_org = |p: Project| -> Result<Level> {
        let org = db.org_of(&p)?;
        Ok(Level::Project(p, org))
    };
    match owners {
        [] => with_org(current_project(db)?),
        // a project, else an organization of that name
        [one] => match db.find_by_name::<Project>(one)? {
            Some(p) => with_org(p),
            None => Ok(Level::Organization(db.resolve::<Organization>(one)?)),
        },
        [org, project] => with_org(project_in(db, project, Some(org))?),
        _ => unreachable!("parse allows two owners at most"),
    }
}

fn task_of(db: &Db, owners: &[String], task: Option<&str>) -> Result<(Project, Task)> {
    let Some(task) = task else {
        if !owners.is_empty() {
            return Err(IterError::CommandFailed(
                "name the task: <project>/tasks/<task>/...".into(),
            ));
        }
        return current_task(db);
    };
    let project = match owners {
        [] if task.bytes().all(|b| b.is_ascii_digit()) => return locate::task(db, &task.parse()?),
        [] => current_project(db)?,
        [one] => db.resolve::<Project>(one)?,
        [org, project] => project_in(db, project, Some(org))?,
        _ => unreachable!("parse allows two owners at most"),
    };
    match task.parse::<i64>() {
        Ok(_) => {
            let (p, t) = locate::task(db, &task.parse()?)?;
            match p.id == project.id {
                true => Ok((p, t)),
                false => Err(IterError::CommandFailed(format!(
                    "task {task} is not in project '{}'",
                    project.name
                ))),
            }
        }
        Err(_) => {
            let t = locate::task_in(db, &project, task)?;
            Ok((project, t))
        }
    }
}

/// The folder a wiki class lives in at `level`, from the ontology (with the level's extension).
pub fn page_folder(class: &str, kind: LevelKind, level_dir: &std::path::Path) -> Result<String> {
    let extension = std::fs::read_to_string(level_dir.join("wiki").join("ontology.ttl")).ok();
    let onto = Ontology::load(extension.as_deref())?;
    let wanted = class.to_lowercase();
    let places: Vec<_> = onto
        .places
        .iter()
        .filter(|p| p.kind == kind && p.folder.starts_with("wiki/"))
        .collect();
    places
        .iter()
        .find(|p| {
            let local = p
                .class
                .as_str()
                .strip_prefix(NS)
                .unwrap_or(p.class.as_str());
            let folder = p.folder.rsplit('/').next().unwrap_or(&p.folder);
            local.eq_ignore_ascii_case(&wanted) || folder.eq_ignore_ascii_case(&wanted)
        })
        .map(|p| p.folder.clone())
        .ok_or_else(|| {
            let known: Vec<&str> = places
                .iter()
                .map(|p| p.folder.trim_start_matches("wiki/"))
                .collect();
            IterError::CommandFailed(format!(
                "no wiki class '{class}' for {} (the ontology has: {})",
                kind.key(),
                known.join(", ")
            ))
        })
}

/// The file `name` is in `dir`: as written if that file exists, else its [`notes::stem`] (what
/// `new` makes of a name).
fn file_in(dir: &std::path::Path, name: &str) -> Result<PathBuf> {
    let exact = dir.join(format!("{name}.md"));
    if exact.is_file() {
        return Ok(exact);
    }
    let stem = notes::stem(name)
        .ok_or_else(|| IterError::CommandFailed(format!("'{name}' is not a name")))?;
    Ok(dir.join(format!("{stem}.md")))
}

/// Finds the folder (and file) an address leads to. Nothing is created; the file may not exist.
pub fn locate(db: &Db, address: &Address, today: NaiveDate) -> Result<Located> {
    let doc = address
        .doc
        .clone()
        .ok_or_else(|| IterError::CommandFailed("which document?".into()))?;
    let name = address.name.as_deref();
    match doc {
        Doc::Daily(day) => {
            let level = level(db, &address.owners)?;
            let dir = level.dir()?;
            let day = day.unwrap_or(today);
            let file = dir.join(format!("{}.md", day.format("%Y-%m-%d")));
            Ok(Located {
                dir,
                file: Some(file),
            })
        }
        Doc::Page(class) => {
            let level = level(db, &address.owners)?;
            let level_dir = level.dir()?;
            let dir = level_dir.join(page_folder(&class, level.kind(), &level_dir)?);
            let file = name.map(|n| file_in(&dir, n)).transpose()?;
            Ok(Located { dir, file })
        }
        Doc::TaskNote | Doc::Sketch => {
            let (project, task) = task_of(db, &address.owners, address.task.as_deref())?;
            let tdir = taskdir::of(db, &project, &task)?;
            let dir = match doc {
                Doc::Sketch => tdir.sketches_dir(),
                _ => tdir.notes_dir(),
            };
            let file = name.map(|n| file_in(&dir, n)).transpose()?;
            Ok(Located { dir, file })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;

    fn doc(text: &str) -> (Vec<String>, Option<String>, Doc, Option<String>) {
        let a = parse(text).unwrap_or_else(|e| panic!("{text}: {e}"));
        (a.owners, a.task, a.doc.unwrap(), a.name)
    }

    fn s(v: &str) -> String {
        v.to_string()
    }

    #[test]
    fn every_shape_reads() {
        let day = NaiveDate::from_ymd_opt(2026, 11, 18);
        assert_eq!(doc("note"), (vec![], None, Doc::Daily(None), None));
        assert_eq!(
            doc("acme/app/note"),
            (vec![s("acme"), s("app")], None, Doc::Daily(None), None)
        );
        assert_eq!(
            doc("app/2026-11-18"),
            (vec![s("app")], None, Doc::Daily(day), None)
        );
        assert_eq!(
            doc("note/2026-11-18.md"),
            (vec![], None, Doc::Daily(day), None)
        );
        assert_eq!(
            doc("acme/wiki/concepts/backoff"),
            (
                vec![s("acme")],
                None,
                Doc::Page(s("concepts")),
                Some(s("backoff"))
            )
        );
        assert_eq!(
            doc("app/tasks/7/sketch/cause"),
            (vec![s("app")], Some(s("7")), Doc::Sketch, Some(s("cause")))
        );
        assert_eq!(
            doc("tasks/7/notes/ideas"),
            (vec![], Some(s("7")), Doc::TaskNote, Some(s("ideas")))
        );
        assert_eq!(
            doc("sketch/cause"),
            (vec![], None, Doc::Sketch, Some(s("cause")))
        );
        assert_eq!(
            doc("note/ideas"),
            (vec![], None, Doc::TaskNote, Some(s("ideas")))
        );
    }

    #[test]
    fn no_address_leaves_its_folder() {
        for text in [
            "sketch/../plan",
            "sketch/.hidden",
            "wiki/concepts/a/b",
            "a/b/c/note",
            "app/nothing",
            "sketch/a\\b",
            "",
        ] {
            assert!(parse(text).is_err(), "{text}");
        }
    }

    #[test]
    fn addresses_lead_to_the_notes_folders() {
        let w = fixtures::world("note-ref");
        let today = fixtures::day("2026-11-18");
        let at = |text: &str| {
            let located = locate(&w.db, &parse(text).unwrap(), today).unwrap();
            located
                .file
                .unwrap()
                .strip_prefix(&w.notes)
                .unwrap()
                .display()
                .to_string()
        };
        let id = w.task_id;
        assert_eq!(at("app/note"), "2026-11-18.md");
        assert_eq!(at("app/wiki/concept/backoff"), "wiki/concepts/backoff.md");
        assert_eq!(at("app/wiki/Synthesis/x"), "wiki/syntheses/x.md");
        assert_eq!(
            at(&format!("tasks/{id}/sketch/Root Cause")),
            format!("tasks/{id}/sketches/root-cause.md")
        );
        assert_eq!(
            at(&format!("app/tasks/Fix it/note/a")),
            format!("tasks/{id}/notes/a.md")
        );
        // a class the ontology does not place at the project level
        assert!(locate(&w.db, &parse("app/wiki/plan/x").unwrap(), today).is_err());
        let _ = std::fs::remove_dir_all(&w.notes);
    }
}
