//! Daily notes: one markdown file per day, `YYYY-MM-DD.md`, in a folder an
//! organization or a project names (`notes_path`). The files are the only
//! store; nothing is kept in the database.
//!
//! A note is the day, a `title` (`dd/mm` unless the file says otherwise), a
//! `layout` and the markdown body below the front matter:
//!
//! ```text
//! layout: page
//! title: 29/10
//! date: 2026-10-29
//! ---
//! what happened today
//! ```
//!
//! The settings come first, one per line, closed by a `---` line; what
//! follows is the description. Reading is forgiving -- a journal written by
//! hand drifts: an opening `---` is accepted too, the header may be missing
//! altogether, and the body may be empty. The day is always the file name's, so a file whose name is
//! not a date is not a note.

use crate::config::expand_home;
use crate::error::Result;
use crate::git::slugify;
use crate::models::{Organization, Project};
use chrono::NaiveDate;
use serde::Deserialize;
use std::io::Write;
use std::path::{Path, PathBuf};

/// The layout a note has when its front matter does not say.
pub const DEFAULT_LAYOUT: &str = "page";

#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    pub date: NaiveDate,
    pub title: String,
    pub layout: String,
    /// Markdown.
    pub body: String,
}

/// The fields a note's front matter may set; any other key is ignored.
#[derive(Deserialize, Default)]
struct FrontMatter {
    title: Option<String>,
    layout: Option<String>,
    date: Option<String>,
}

impl FrontMatter {
    fn is_empty(&self) -> bool {
        self.title.is_none() && self.layout.is_none() && self.date.is_none()
    }
}

/// `dd/mm`: what a note is called unless it says otherwise.
pub fn default_title(date: NaiveDate) -> String {
    date.format("%d/%m").to_string()
}

/// The day `text` is in `YYYY-MM-DD` form, exactly: this is the only way
/// from a URL or a file name to a note, so no other text reaches a path.
pub fn parse_date(text: &str) -> Option<NaiveDate> {
    let shaped = text.len() == 10
        && text.bytes().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                b == b'-'
            } else {
                b.is_ascii_digit()
            }
        });
    shaped
        .then(|| NaiveDate::parse_from_str(text, "%Y-%m-%d").ok())
        .flatten()
}

fn file_name(date: NaiveDate) -> String {
    format!("{}.md", date.format("%Y-%m-%d"))
}

/// The front matter and body of `content`. With the opening `---` the
/// front matter is whatever lies before the next `---` line. Without it, a
/// leading block counts as front matter only when it reads as one (YAML that
/// sets a known key), so a note that starts with prose and has a `---` rule
/// further down is left whole.
fn split(content: &str) -> (FrontMatter, &str) {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let (opened, rest) = match content.strip_prefix("---\n") {
        Some(rest) => (true, rest),
        None => (false, content),
    };
    let Some((front, body)) = crate::md_edit::split_at_divider(rest) else {
        return (FrontMatter::default(), content);
    };
    match serde_yaml::from_str::<FrontMatter>(front) {
        Ok(fm) if opened || !fm.is_empty() => (fm, body),
        _ if opened => (FrontMatter::default(), body),
        _ => (FrontMatter::default(), content),
    }
}

/// The note of `date` written as `content`.
pub fn parse(date: NaiveDate, content: &str) -> Note {
    let (front, body) = split(content);
    let _ = front.date; // the file name is the day; a date in the file may be stale
    Note {
        date,
        title: front
            .title
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| default_title(date)),
        layout: front
            .layout
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .unwrap_or_else(|| DEFAULT_LAYOUT.to_string()),
        body: body.trim().to_string(),
    }
}

/// The notes in `dir`, the latest day first. A folder that is not there yet
/// has none; a file that cannot be read is left out.
pub fn list(dir: &Path) -> Vec<Note> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut notes: Vec<Note> = entries
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            let date = note_day(&path)?;
            let content = std::fs::read_to_string(&path).ok()?;
            Some(parse(date, &content))
        })
        .collect();
    notes.sort_by_key(|n| std::cmp::Reverse(n.date));
    notes
}

/// How many notes `dir` holds (the files [`list`] would read, unread).
pub fn count(dir: &Path) -> usize {
    std::fs::read_dir(dir).map_or(0, |entries| {
        entries
            .filter_map(|e| e.ok())
            .filter(|e| is_note_file(&e.path()))
            .count()
    })
}

/// A file named `YYYY-MM-DD.md`: its day, if so.
fn note_day(path: &Path) -> Option<NaiveDate> {
    parse_date(path.file_stem()?.to_str()?).filter(|_| path.extension().is_some_and(|e| e == "md"))
}

fn is_note_file(path: &Path) -> bool {
    note_day(path).is_some()
}

/// The note of `date` in `dir`, if there is one.
#[cfg(test)]
fn get(dir: &Path, date: NaiveDate) -> Option<Note> {
    let content = std::fs::read_to_string(dir.join(file_name(date))).ok()?;
    Some(parse(date, &content))
}

/// Makes sure the note of `day` exists, creating the folder and a file with
/// only its front matter if need be. A note already there is left alone.
pub fn create(dir: &Path, day: NaiveDate) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dir.join(file_name(day)));
    match file {
        Ok(mut file) => {
            let front = format!(
                "layout: {DEFAULT_LAYOUT}\ntitle: {}\ndate: {}\n---\n\n",
                default_title(day),
                day.format("%Y-%m-%d")
            );
            file.write_all(front.as_bytes())?;
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// The days either side of `date` among `notes` (latest first): the
/// previous (older) and the next (newer) one.
pub fn neighbours(notes: &[Note], date: NaiveDate) -> (Option<&Note>, Option<&Note>) {
    let at = notes.iter().position(|n| n.date == date);
    let Some(at) = at else { return (None, None) };
    let newer = at.checked_sub(1).and_then(|i| notes.get(i));
    (notes.get(at + 1), newer)
}

fn resolve(path: &str) -> Option<PathBuf> {
    let path = path.trim();
    (!path.is_empty()).then(|| expand_home(path))
}

/// Where an organization keeps its notes, if it says.
pub fn org_dir(org: &Organization) -> Option<PathBuf> {
    resolve(&org.notes_path)
}

/// Where a project keeps its notes: its own folder if it names one, else a
/// folder of its name inside its organization's.
pub fn project_dir(project: &Project, org: &Option<Organization>) -> Option<PathBuf> {
    resolve(&project.notes_path).or_else(|| {
        let root = org_dir(org.as_ref()?)?;
        let slug = slugify(&project.name);
        let slug = if slug.is_empty() {
            format!("project-{}", project.id.unwrap_or_default())
        } else {
            slug
        };
        Some(root.join(slug))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(d: &str) -> NaiveDate {
        parse_date(d).expect("a date")
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("iter-notes-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn only_a_whole_iso_date_is_a_day() {
        assert!(parse_date("2026-10-29").is_some());
        for bad in [
            "",
            "2026-1-29",
            "../../etc",
            "2026-13-01",
            "2026-10-29x",
            "+026-10-29",
        ] {
            assert_eq!(parse_date(bad), None, "{bad}");
        }
    }

    #[test]
    fn front_matter_sets_title_and_layout() {
        let n = parse(
            day("2026-10-29"),
            "---\ntitle: Kickoff\nlayout: preview\nauthor: me\ndate: 2020-01-01\n---\n\nhello\n",
        );
        assert_eq!(
            (n.title.as_str(), n.layout.as_str(), n.body.as_str()),
            ("Kickoff", "preview", "hello")
        );
        assert_eq!(n.date, day("2026-10-29"));
    }

    #[test]
    fn a_note_without_front_matter_gets_the_defaults() {
        let n = parse(day("2026-10-29"), "- one\n- two\n\n---\n\nrule above\n");
        assert_eq!((n.title.as_str(), n.layout.as_str()), ("29/10", "page"));
        assert!(n.body.starts_with("- one") && n.body.ends_with("rule above"));
        assert_eq!(parse(day("2026-10-29"), "").body, "");
    }

    #[test]
    fn a_created_note_has_the_header_without_an_opening_divider() {
        let dir = scratch("header");
        create(&dir, day("2026-10-29")).expect("creates");
        let text = std::fs::read_to_string(dir.join("2026-10-29.md")).expect("read");
        assert_eq!(
            text,
            "layout: page\ntitle: 29/10\ndate: 2026-10-29\n---\n\n"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_opening_divider_is_forgiven() {
        let n = parse(day("2026-10-12"), "title: 12/10\nlayout: page\n---\nbody\n");
        assert_eq!((n.title.as_str(), n.body.as_str()), ("12/10", "body"));
    }

    #[test]
    fn notes_list_latest_first_and_skip_other_files() {
        let dir = scratch("list");
        std::fs::create_dir_all(&dir).expect("dir");
        for (name, text) in [
            ("2026-10-27.md", "a"),
            ("2026-10-29.md", "c"),
            ("2026-10-28.md", "b"),
            ("readme.md", "x"),
            ("2026-10-30.txt", "x"),
        ] {
            std::fs::write(dir.join(name), text).expect("write");
        }
        let notes = list(&dir);
        let days: Vec<_> = notes.iter().map(|n| n.date.to_string()).collect();
        assert_eq!(days, ["2026-10-29", "2026-10-28", "2026-10-27"]);
        let (prev, next) = neighbours(&notes, day("2026-10-28"));
        assert_eq!(
            (prev.map(|n| n.body.as_str()), next.map(|n| n.body.as_str())),
            (Some("a"), Some("c"))
        );
        assert!(list(&dir.join("missing")).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn creating_a_day_twice_keeps_what_was_written() {
        let dir = scratch("create");
        create(&dir, day("2026-10-29")).expect("creates");
        let first = get(&dir, day("2026-10-29")).expect("exists");
        assert_eq!((first.title.as_str(), first.body.as_str()), ("29/10", ""));
        std::fs::write(dir.join("2026-10-29.md"), "written").expect("write");
        create(&dir, day("2026-10-29")).expect("again");
        assert_eq!(get(&dir, day("2026-10-29")).expect("kept").body, "written");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_project_without_a_folder_uses_its_organizations() {
        let org = Organization {
            notes_path: "/notes".into(),
            ..Organization::default()
        };
        let project = Project {
            name: "My App".into(),
            ..Project::default()
        };
        assert_eq!(
            project_dir(&project, &Some(org.clone())),
            Some(PathBuf::from("/notes/my-app"))
        );
        assert_eq!(project_dir(&project, &None), None);
        let own = Project {
            notes_path: "/mine".into(),
            ..project
        };
        assert_eq!(project_dir(&own, &Some(org)), Some(PathBuf::from("/mine")));
    }
}
