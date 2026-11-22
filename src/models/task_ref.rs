//! How a task is written and read back: the id (`42`, `#42`), the label iter prints
//! (`web/login #42`) and the title reference (`web/login`). What iter prints, it reads.

use crate::error::{IterError, Result};
use crate::models::{Project, Table, Task};
use std::fmt;
use std::str::FromStr;

/// A task id as written: `#42`, read from `42` or `#42`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TaskId(i64);

impl TaskId {
    pub fn get(self) -> i64 {
        self.0
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

impl FromStr for TaskId {
    type Err = IterError;
    fn from_str(text: &str) -> Result<Self> {
        digits(text.strip_prefix('#').unwrap_or(text))
            .map(TaskId)
            .ok_or_else(|| IterError::InvalidTaskRef(text.to_string()))
    }
}

/// The value of a non-empty run of ASCII digits that fits an `i64`.
fn digits(text: &str) -> Option<i64> {
    match !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()) {
        true => text.parse().ok(),
        false => None,
    }
}

/// The title and id of a `<title> #<id>` text. A trailing `#<digits>` is always the id, also
/// where a task is titled literally `fix #12`: that one is reached by its own id.
fn split_label(text: &str) -> Option<(&str, i64)> {
    let (title, id) = text.rsplit_once(" #")?;
    Some((title, digits(id)?))
}

/// A task as the CLI and the web show it: `<project>/<title> #<id>`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TaskLabel {
    pub id: i64,
    pub project: String,
    pub title: String,
}

impl TaskLabel {
    /// Whether the title ends in ` #<digits>`, so that `<project>/<title>` would be read back as
    /// a label naming another task.
    pub fn title_reads_as_label(&self) -> bool {
        split_label(&self.title).is_some()
    }
}

impl fmt::Display for TaskLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{} {}", self.project, self.title, TaskId(self.id))
    }
}

impl Task {
    /// The slug of the task's branch and worktree folder: `<id>-<title slug>`, or `task-<id>` when
    /// the title has no ASCII letter or digit (replaces `git::task_slug`).
    #[allow(dead_code)] // stub: I4
    pub fn slug(&self) -> String {
        unimplemented!("stub: I4")
    }

    pub fn label(&self, project: &Project) -> TaskLabel {
        TaskLabel {
            id: self.id(),
            project: project.name.clone(),
            title: self.name.clone(),
        }
    }
}

/// What a person types to name a task.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TaskRef {
    /// `42` or `#42`.
    Id(TaskId),
    /// `web/login`.
    Title { project: String, title: String },
    /// `web/login #42`: the id wins over a stale title.
    Label(TaskLabel),
}

impl FromStr for TaskRef {
    type Err = IterError;
    fn from_str(text: &str) -> Result<Self> {
        if let Ok(id) = text.parse() {
            return Ok(TaskRef::Id(id));
        }
        let (project, rest) = text
            .split_once('/')
            .ok_or_else(|| IterError::InvalidTaskRef(text.to_string()))?;
        let project = project.to_string();
        Ok(match split_label(rest) {
            Some((title, id)) => TaskRef::Label(TaskLabel {
                id,
                project,
                title: title.to_string(),
            }),
            None => TaskRef::Title {
                project,
                title: rest.to_string(),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;

    fn label(id: i64, project: &str, title: &str) -> TaskLabel {
        TaskLabel {
            id,
            project: project.into(),
            title: title.into(),
        }
    }

    fn title(project: &str, title: &str) -> TaskRef {
        TaskRef::Title {
            project: project.into(),
            title: title.into(),
        }
    }

    fn id(n: i64) -> TaskRef {
        TaskRef::Id(TaskId(n))
    }

    #[test]
    fn an_id_is_shown_with_a_hash() {
        assert_eq!(TaskId(42).to_string(), "#42");
    }

    #[test]
    fn an_id_reads_with_or_without_the_hash() {
        for (text, n) in [("42", 42), ("#42", 42), ("0", 0), ("#007", 7)] {
            assert_eq!(text.parse::<TaskId>().unwrap(), TaskId(n), "{text}");
        }
    }

    #[test]
    fn what_is_not_an_id_is_refused() {
        for text in [
            "",
            "#",
            "##4",
            "4x",
            "-1",
            "+1",
            "# 42",
            "42 ",
            " 42",
            "web/login",
            "1.5",
        ] {
            assert!(
                matches!(text.parse::<TaskId>(), Err(IterError::InvalidTaskRef(t)) if t == text),
                "{text:?}"
            );
        }
    }

    /// An id too large for `i64` is refused, not wrapped.
    #[test]
    fn an_id_beyond_the_range_is_refused() {
        assert!("99999999999999999999".parse::<TaskId>().is_err());
    }

    #[test]
    fn a_label_is_project_slash_title_and_the_id() {
        assert_eq!(label(42, "web", "login").to_string(), "web/login #42");
        assert_eq!(label(3, "web", "a/b c").to_string(), "web/a/b c #3");
    }

    #[test]
    fn a_task_makes_its_label_with_its_project() {
        let project = fixtures::project("web");
        let task = Task {
            id: Some(42),
            ..fixtures::task(1, "login")
        };
        assert_eq!(task.label(&project), label(42, "web", "login"));
        assert_eq!(task.label(&project).to_string(), "web/login #42");
    }

    #[test]
    fn a_reference_reads_in_each_of_its_three_forms() {
        let cases = [
            ("42", id(42)),
            ("#42", id(42)),
            ("web/login", title("web", "login")),
            ("web/login #42", TaskRef::Label(label(42, "web", "login"))),
            // the title keeps everything after the first slash
            ("web/a/b", title("web", "a/b")),
            ("web/with space", title("web", "with space")),
            // a trailing `#<id>` is always the id, even where it reads like part of a title
            ("web/fix #12", TaskRef::Label(label(12, "web", "fix"))),
            ("web/fix #12 #7", TaskRef::Label(label(7, "web", "fix #12"))),
            // not digits after the hash: part of the title
            ("web/fix #12x", title("web", "fix #12x")),
            ("web/fix #", title("web", "fix #")),
            ("web/fix#12", title("web", "fix#12")),
        ];
        for (text, expected) in cases {
            assert_eq!(text.parse::<TaskRef>().unwrap(), expected, "{text}");
        }
    }

    #[test]
    fn a_reference_with_no_project_or_id_is_refused() {
        for text in ["", "login", "a b", "#", "#x"] {
            assert!(
                matches!(text.parse::<TaskRef>(), Err(IterError::InvalidTaskRef(t)) if t == text),
                "{text:?}"
            );
        }
    }

    /// What iter prints is what iter reads: the label of any task, whatever its title holds.
    #[test]
    fn a_label_reads_back_as_itself() {
        for (n, project, name) in [
            (1, "web", "login"),
            (42, "web", "fix #12"),
            (7, "my-org.app", "a/b/c"),
            (9, "web", "-starts with a dash"),
            (10, "web", "ends with hash #"),
            (11, "web", "ünïcode ✓"),
        ] {
            let shown = label(n, project, name);
            assert_eq!(
                shown.to_string().parse::<TaskRef>().unwrap(),
                TaskRef::Label(shown),
            );
        }
    }

    /// Round I4 (isolation step 6): the slug is the task's own, the same cases as `git::task_slug`.
    #[test]
    fn the_slug_of_a_task_carries_its_id() {
        let task = |id: i64, name: &str| Task {
            id: Some(id),
            ..fixtures::task(1, name)
        };
        for (id, name, slug) in [
            (42, "Fix Login Bug!", "42-fix-login-bug"),
            (12, "login", "12-login"),
            (42, "設計を見直す", "task-42"),
            (7, "!!! ---", "task-7"),
            (3, "", "task-3"),
        ] {
            assert_eq!(task(id, name).slug(), slug, "{id} {name:?}");
        }
        assert_ne!(task(1, "login").slug(), task(2, "login").slug());
    }
}
