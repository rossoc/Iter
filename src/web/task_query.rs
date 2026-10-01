//! The query box of a task list (`?q=`): a few GitHub-style terms in one
//! text field, parsed here and matched against tasks. Pure, no markup.
//!
//! | term | meaning |
//! |---|---|
//! | `is:open` | queued or work in progress (the default query: what the links into a list carry) |
//! | `is:done`, `is:wip`, `is:queue` | one status |
//! | `is:all` | every status; so is a query with no `is:` term, and an empty one (no `q`: the server reads an empty `q=` as none) |
//! | `tag:name`, `tag:"two words"` | has the tag (case-insensitive) |
//! | `project:name` | the project's name contains it |
//! | `-term` | the opposite of any term (`-tag:agent`) |
//! | anything else | the task's name contains it; `#12` is the issue |
//!
//! A term is never an error: an unknown `key:value` is text to look for in
//! the name.

use crate::db::Db;
use crate::models::{Task, TaskStatus};

/// What a list starts with: the tasks that are not done. The links into a
/// list spell it out (`url::tasks_tab_url`); no `q` at all is no filter.
pub const DEFAULT: &str = "is:open";

#[derive(Debug, PartialEq)]
enum Kind {
    /// Any of these statuses.
    Status(Vec<TaskStatus>),
    /// A tag's name, lowercased.
    Tag(String),
    /// Part of a project's name, lowercased.
    Project(String),
    /// Part of the task's name, lowercased.
    Name(String),
    /// The task's GitHub issue.
    Issue(i64),
}

#[derive(Debug, PartialEq)]
struct Term {
    negated: bool,
    kind: Kind,
}

/// A parsed query: every term must hold.
#[derive(Debug, PartialEq)]
pub struct TaskQuery {
    terms: Vec<Term>,
}

/// Splits `q` into words on blanks; a `"` opens or closes a quoted stretch
/// (blanks inside it stay) and is dropped.
fn words(q: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    let mut quoted = false;
    let mut started = false;
    for c in q.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            c if c.is_whitespace() && !quoted => {
                if started {
                    out.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            c => {
                word.push(c);
                started = true;
            }
        }
    }
    if started {
        out.push(word);
    }
    out
}

fn status_of(value: &str) -> Option<Vec<TaskStatus>> {
    match value.to_lowercase().as_str() {
        "open" => Some(vec![TaskStatus::Queue, TaskStatus::Wip]),
        "all" => Some(TaskStatus::ALL.to_vec()),
        "done" | "closed" => Some(vec![TaskStatus::Done]),
        "wip" => Some(vec![TaskStatus::Wip]),
        "queue" | "queued" => Some(vec![TaskStatus::Queue]),
        _ => None,
    }
}

fn kind_of(word: &str) -> Option<Kind> {
    if word.is_empty() {
        return None;
    }
    if let Some((key, value)) = word.split_once(':') {
        let value = value.trim();
        match key.to_lowercase().as_str() {
            "is" => {
                if let Some(statuses) = status_of(value) {
                    return Some(Kind::Status(statuses));
                }
            }
            "tag" if !value.is_empty() => return Some(Kind::Tag(value.to_lowercase())),
            "project" if !value.is_empty() => return Some(Kind::Project(value.to_lowercase())),
            _ => {}
        }
    }
    if let Some(issue) = word.strip_prefix('#').and_then(|n| n.parse().ok()) {
        return Some(Kind::Issue(issue));
    }
    Some(Kind::Name(word.to_lowercase()))
}

impl TaskQuery {
    /// The query `q` says; blank is everything.
    pub fn parse(q: &str) -> TaskQuery {
        let terms = words(q)
            .iter()
            .filter_map(|word| {
                // a lone `-` has nothing to negate: it is skipped
                let (negated, rest) = match word.strip_prefix('-') {
                    Some(rest) if !rest.is_empty() => (true, rest),
                    Some(_) => return None,
                    None => (false, word.as_str()),
                };
                kind_of(rest).map(|kind| Term { negated, kind })
            })
            .collect();
        TaskQuery { terms }
    }

    /// The query of a `?q=`: absent (or empty) means every task.
    pub fn of(q: Option<&str>) -> TaskQuery {
        TaskQuery::parse(q.unwrap_or(""))
    }

    /// What the search box shows for a `?q=`.
    pub fn text(q: Option<&str>) -> &str {
        q.unwrap_or("")
    }

    /// Whether the query asks about tags (so they are worth loading).
    pub fn needs_tags(&self) -> bool {
        self.terms.iter().any(|t| matches!(t.kind, Kind::Tag(_)))
    }

    /// Whether `task` of `project` with the tag names `tags` passes every term.
    pub fn matches(&self, task: &Task, project: &str, tags: &[String]) -> bool {
        self.terms.iter().all(|term| {
            let holds = match &term.kind {
                Kind::Status(statuses) => statuses.contains(&task.status),
                Kind::Tag(name) => tags.iter().any(|t| t.to_lowercase() == *name),
                Kind::Project(part) => project.to_lowercase().contains(part),
                Kind::Name(part) => task.name.to_lowercase().contains(part),
                Kind::Issue(n) => task.github_issue == Some(*n),
            };
            holds != term.negated
        })
    }

    /// For each `(task, project name)`, whether it passes. Tags are read
    /// (one query) only when the query asks about them.
    fn keep(&self, db: &Db, items: &[(&Task, &str)]) -> crate::error::Result<Vec<bool>> {
        let tags = if self.needs_tags() {
            let ids: Vec<i64> = items.iter().filter_map(|(t, _)| t.id).collect();
            db.tag_names_for_tasks(&ids)?
        } else {
            Default::default()
        };
        Ok(items
            .iter()
            .map(|(task, project)| {
                let mine = task
                    .id
                    .and_then(|id| tags.get(&id))
                    .map_or(&[][..], Vec::as_slice);
                self.matches(task, project, mine)
            })
            .collect())
    }

    /// The tasks of one project (named `project`) that pass, in order.
    pub fn filter_tasks(
        &self,
        db: &Db,
        tasks: Vec<Task>,
        project: &str,
    ) -> crate::error::Result<Vec<Task>> {
        let items: Vec<(&Task, &str)> = tasks.iter().map(|t| (t, project)).collect();
        let keep = self.keep(db, &items)?;
        Ok(tasks
            .into_iter()
            .zip(keep)
            .filter_map(|(t, keep)| keep.then_some(t))
            .collect())
    }

    /// The `(project name, task)` rows of a group that pass, in order.
    pub fn filter_named(
        &self,
        db: &Db,
        rows: Vec<(String, Task)>,
    ) -> crate::error::Result<Vec<(String, Task)>> {
        let items: Vec<(&Task, &str)> = rows.iter().map(|(p, t)| (t, p.as_str())).collect();
        let keep = self.keep(db, &items)?;
        Ok(rows
            .into_iter()
            .zip(keep)
            .filter_map(|(row, keep)| keep.then_some(row))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(name: &str, status: TaskStatus, issue: Option<i64>) -> Task {
        let mut t = Task::template(1, String::new(), status);
        t.name = name.into();
        t.github_issue = issue;
        t
    }

    fn tags(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    fn hits(q: &str, t: &Task, project: &str, tag: &[&str]) -> bool {
        TaskQuery::parse(q).matches(t, project, &tags(tag))
    }

    #[test]
    fn words_split_on_blanks_and_keep_quoted_stretches() {
        assert_eq!(words("a  b"), ["a", "b"]);
        assert_eq!(words(r#"tag:"two words" x"#), ["tag:two words", "x"]);
        assert_eq!(words(r#""a b""#), ["a b"]);
        assert_eq!(words(r#"-tag:"a b""#), ["-tag:a b"]);
        assert!(words("   ").is_empty());
    }

    #[test]
    fn the_default_is_open_and_no_query_is_everything() {
        let open = task("a", TaskStatus::Wip, None);
        let done = task("a", TaskStatus::Done, None);
        let default = TaskQuery::of(Some(DEFAULT));
        assert!(default.matches(&open, "p", &[]) && !default.matches(&done, "p", &[]));
        for all in [
            TaskQuery::of(None),
            TaskQuery::of(Some("")),
            TaskQuery::of(Some("is:all")),
        ] {
            assert!(all.matches(&open, "p", &[]) && all.matches(&done, "p", &[]));
        }
        // no `is:` term: every status
        assert!(TaskQuery::of(Some("a")).matches(&done, "p", &[]));
        assert_eq!(TaskQuery::text(None), "");
        assert_eq!(TaskQuery::text(Some("is:open")), "is:open");
    }

    #[test]
    fn statuses() {
        let q = task("a", TaskStatus::Queue, None);
        let w = task("a", TaskStatus::Wip, None);
        let d = task("a", TaskStatus::Done, None);
        assert!(hits("is:queue", &q, "", &[]) && !hits("is:queue", &w, "", &[]));
        assert!(hits("is:wip", &w, "", &[]) && !hits("is:wip", &d, "", &[]));
        assert!(hits("is:done", &d, "", &[]) && !hits("is:done", &q, "", &[]));
        assert!(hits("IS:Open", &w, "", &[]));
        assert!(hits("-is:done", &w, "", &[]) && !hits("-is:done", &d, "", &[]));
    }

    #[test]
    fn tags_are_matched_whole_and_case_insensitively() {
        let t = task("a", TaskStatus::Queue, None);
        assert!(hits("tag:agent", &t, "", &["Agent", "x"]));
        assert!(!hits("tag:age", &t, "", &["Agent"]));
        assert!(hits(r#"tag:"two words""#, &t, "", &["Two Words"]));
        assert!(hits("-tag:agent", &t, "", &["x"]));
        assert!(!hits("-tag:agent", &t, "", &["agent"]));
        assert!(TaskQuery::parse("tag:x").needs_tags());
        assert!(!TaskQuery::parse("is:open x").needs_tags());
    }

    #[test]
    fn project_and_free_text() {
        let t = task("Fix the Login bug", TaskStatus::Queue, Some(12));
        assert!(hits("project:app", &t, "my-App", &[]));
        assert!(!hits("project:web", &t, "my-App", &[]));
        assert!(hits("login", &t, "", &[]));
        assert!(hits("fix bug", &t, "", &[]), "words are ANDed");
        assert!(!hits("fix nothing", &t, "", &[]));
        assert!(hits("-nothing", &t, "", &[]));
        assert!(hits("#12", &t, "", &[]) && !hits("#13", &t, "", &[]));
        assert!(
            hits(r#""the login""#, &t, "", &[]),
            "a quoted phrase is one term"
        );
    }

    #[test]
    fn unknown_terms_are_name_text_never_errors() {
        let t = task("see http://x and foo:bar", TaskStatus::Queue, None);
        assert!(hits("foo:bar", &t, "", &[]));
        assert!(hits(
            "is:weird",
            &task("is:weird", TaskStatus::Queue, None),
            "",
            &[]
        ));
        assert!(!hits("is:weird", &t, "", &[]));
        // bare key with nothing after it is text too
        assert!(hits(
            "tag:",
            &task("x tag: y", TaskStatus::Queue, None),
            "",
            &[]
        ));
        // a lone dash is skipped
        assert_eq!(TaskQuery::parse("-"), TaskQuery::parse(""));
    }

    #[test]
    fn filter_reads_tags_only_when_asked() {
        let db = Db::open(":memory:").expect("db");
        let mut p = crate::models::Project::template(&crate::models::Settings::default());
        p.name = "p".into();
        p.base_path = "/tmp/p".into();
        let project = db.insert(&p).expect("project");
        let mut ids = Vec::new();
        for n in ["one", "two"] {
            let mut t = Task::template(project, String::new(), TaskStatus::Queue);
            t.name = n.into();
            ids.push(db.insert(&t).expect("task"));
        }
        let agent = db
            .ids_by_name::<crate::models::Tag>(&["Urgent".to_string()])
            .expect("tag");
        db.set_task_tags(ids[0], &agent).expect("tagged");
        let rows = db.tasks_for_project(project).expect("rows");
        let names = |q: &str| -> Vec<String> {
            TaskQuery::parse(q)
                .filter_tasks(&db, rows.clone(), "p")
                .expect("filtered")
                .into_iter()
                .map(|t| t.name)
                .collect()
        };
        assert_eq!(names("tag:urgent"), ["one"]);
        assert_eq!(names("-tag:urgent"), ["two"]);
        assert_eq!(names(""), ["one", "two"]);
    }
}
