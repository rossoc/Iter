//! How a task is written and read back: the id (`42`, `#42`), the label iter prints
//! (`web/login #42`) and the title reference (`web/login`).
//!
//! stub: I2 (task 298). The types and signatures are the design's (sketch `architecture`,
//! section 5); every body is `unimplemented!()` until the develop step of round I2, and the tests
//! below fail until then.
#![allow(dead_code)]

use crate::error::{IterError, Result};
use crate::models::{Project, Task};
use std::fmt;
use std::str::FromStr;

/// A task id as written: `#42`, read from `42` or `#42`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TaskId(i64);

impl fmt::Display for TaskId {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        unimplemented!("stub: I2")
    }
}

impl FromStr for TaskId {
    type Err = IterError;
    fn from_str(_: &str) -> Result<Self> {
        unimplemented!("stub: I2")
    }
}

/// A task as the CLI and the web show it: `<project>/<title> #<id>`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TaskLabel {
    pub id: i64,
    pub project: String,
    pub title: String,
}

impl fmt::Display for TaskLabel {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        unimplemented!("stub: I2")
    }
}

impl Task {
    pub fn label(&self, _project: &Project) -> TaskLabel {
        unimplemented!("stub: I2")
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
    fn from_str(_: &str) -> Result<Self> {
        unimplemented!("stub: I2")
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
}
