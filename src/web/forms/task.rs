//! The task form, shared by Edit task and New task.

use super::{names, optional, text};
use crate::db::Db;
use crate::error::{IterError, Result};
use crate::models::{Duration, Tag, Task, TaskStatus, parse_start_time, require_name};
use crate::web::edit::{EditForm, name_field};
use crate::web::url::task_url;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct TaskForm {
    pub name: String,
    pub description: String,
    /// The issue number, or empty for none.
    pub github_issue: String,
    pub status: String,
    pub branch_prefix: String,
    /// `yyyy-mm-dd hh:mm`, or empty for unscheduled.
    pub start_time: String,
    /// `hh:mm`, or empty for none.
    pub duration: String,
    /// Tag names, comma-separated.
    pub tags: String,
}

impl TaskForm {
    /// The names of the controls an error can be about (also the names of
    /// the submitted fields), shared by [`EditForm::field`] and the page.
    pub const BRANCH_PREFIX: &'static str = "branch_prefix";
    pub const STATUS: &'static str = "status";
    pub const GITHUB_ISSUE: &'static str = "github_issue";
    pub const START_TIME: &'static str = "start_time";
    pub const DURATION: &'static str = "duration";
    pub const TAGS: &'static str = "tags";

    /// The form as it starts out for `task` with the tags named `tags`: each
    /// input as the text it shows. A refused submission is shown again as
    /// itself, so what was typed (also an issue number that doesn't parse)
    /// is what comes back.
    pub fn of(task: &Task, tags: &str) -> TaskForm {
        TaskForm {
            name: task.name.clone(),
            description: task.description.clone(),
            github_issue: task.github_issue.map(|n| n.to_string()).unwrap_or_default(),
            status: task.status.as_str().to_string(),
            branch_prefix: task.branch_prefix.clone(),
            start_time: task.start_text(),
            duration: task.duration_text(),
            tags: tags.to_string(),
        }
    }

    /// The tags the form names, resolved first so an unknown name writes
    /// nothing.
    fn tag_ids(&self, db: &Db) -> Result<Vec<i64>> {
        db.ids_by_name::<Tag>(&names(&self.tags))
    }
}

/// How the messages of `TaskForm::apply` begin, which is how
/// `TaskForm::field` tells which field one is about.
const BAD_ISSUE: &str = "GitHub issue";
const BAD_START: &str = "Start time";
const BAD_DURATION: &str = "Duration";

impl EditForm for TaskForm {
    type Row = Task;

    fn field(error: &IterError) -> Option<&'static str> {
        match error {
            IterError::InvalidStatus(_) => Some(Self::STATUS),
            IterError::NotFound { kind: "tag", .. } => Some(Self::TAGS),
            IterError::CommandFailed(m) if m.starts_with(BAD_ISSUE) => Some(Self::GITHUB_ISSUE),
            IterError::CommandFailed(m) if m.starts_with(BAD_START) => Some(Self::START_TIME),
            IterError::CommandFailed(m) if m.starts_with(BAD_DURATION) => Some(Self::DURATION),
            _ => name_field(error),
        }
    }

    fn saved(id: i64) -> String {
        task_url(id)
    }

    /// A field that doesn't parse keeps the row's old value; the first such
    /// error is the one reported.
    fn apply(&self, mut task: Task) -> (Task, Result<()>) {
        let mut valid = require_name("task", &self.name);
        valid = valid.and(optional(
            &mut task.github_issue,
            &self.github_issue,
            |s| s.parse().ok(),
            |raw| {
                IterError::CommandFailed(format!(
                    "{BAD_ISSUE} '{raw}' is not valid. Use a whole number such as 42."
                ))
            },
        ));
        match TaskStatus::parse(&self.status) {
            Some(status) => task.status = status,
            None => valid = valid.and(Err(IterError::InvalidStatus(self.status.clone()))),
        }
        valid = valid.and(optional(
            &mut task.start_time,
            &self.start_time,
            parse_start_time,
            |raw| {
                IterError::CommandFailed(format!(
                    "{BAD_START} '{raw}' is not valid. Use 2026-09-29 14:30."
                ))
            },
        ));
        valid = valid.and(optional(
            &mut task.duration,
            &self.duration,
            Duration::parse,
            |raw| {
                IterError::CommandFailed(format!(
                    "{BAD_DURATION} '{raw}' is not valid. Use hours:minutes, such as 01:30."
                ))
            },
        ));
        task.name = self.name.trim().to_string();
        task.description = text(&self.description);
        task.branch_prefix = self.branch_prefix.trim().to_string();
        (task, valid)
    }

    /// The tags too, resolved first so an unknown name writes nothing (so an
    /// unknown tag is reported before a taken name: the name is only checked
    /// by the write).
    fn save(&self, db: &Db, id: i64, task: &Task) -> Result<()> {
        db.save_task(Some(id), task, &self.tag_ids(db)?)?;
        Ok(())
    }

    /// The new task and its tags, in one transaction.
    fn insert(&self, db: &Db, task: &Task) -> Result<i64> {
        db.save_task(None, task, &self.tag_ids(db)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Project;
    use crate::models::Settings;
    use crate::web::forms::NAME;

    fn task_form(name: &str, issue: &str, status: &str) -> TaskForm {
        TaskForm {
            name: name.into(),
            description: "a\r\nb".into(),
            github_issue: issue.into(),
            status: status.into(),
            branch_prefix: "feat/".into(),
            start_time: String::new(),
            duration: String::new(),
            tags: String::new(),
        }
    }

    fn task() -> Task {
        Task::template(7, String::new(), TaskStatus::Queue)
    }

    /// Each task error that belongs to a field says which.
    #[test]
    fn task_errors_name_their_field() {
        let error = |form: TaskForm| form.apply(task()).1.expect_err("refused");
        let mut form = task_form("x", "abc", "wip");
        assert_eq!(TaskForm::field(&error(form)), Some(TaskForm::GITHUB_ISSUE));
        form = task_form(" ", "", "wip");
        assert_eq!(TaskForm::field(&error(form)), Some(NAME));
        form = task_form("x", "", "nope");
        assert_eq!(TaskForm::field(&error(form)), Some(TaskForm::STATUS));
        form = task_form("x", "", "wip");
        form.start_time = "tomorrow".into();
        assert_eq!(TaskForm::field(&error(form)), Some(TaskForm::START_TIME));
        form = task_form("x", "", "wip");
        form.duration = "1h".into();
        assert_eq!(TaskForm::field(&error(form)), Some(TaskForm::DURATION));
        let db = crate::db::Db::open(":memory:").expect("db");
        let unknown = db
            .ids_by_name::<Tag>(&["typo".to_string()])
            .expect_err("unknown tag");
        assert_eq!(TaskForm::field(&unknown), Some(TaskForm::TAGS));
    }

    /// The form of a task shows each input as text, and a form built from a
    /// task applies back to the same task.
    #[test]
    fn a_task_makes_the_form_that_applies_back_to_it() {
        let mut t = task();
        t.name = "x".into();
        t.github_issue = Some(9);
        t.duration = Some(Duration(90));
        let form = TaskForm::of(&t, "Urgent");
        assert_eq!(
            (form.github_issue.as_str(), form.duration.as_str()),
            ("9", "01:30")
        );
        assert_eq!(form.status, "queue");
        let (back, valid) = form.apply(task());
        assert!(valid.is_ok());
        assert_eq!(
            (back.name, back.github_issue, back.duration),
            ("x".to_string(), Some(9), Some(Duration(90)))
        );
    }

    /// Creating validates, resolves tags and inserts atomically: a refusal
    /// (bad issue, unknown tag) leaves no task behind.
    #[test]
    fn create_writes_the_task_with_its_tags_or_nothing() {
        let db = crate::db::Db::open(":memory:").expect("db");
        let mut p = Project::template(&Settings::default());
        p.name = "p".into();
        p.base_path = "/tmp/p".into();
        let project = db.insert(&p).expect("project");
        let template = || Task::template(project, String::new(), TaskStatus::Queue);
        let mut form = task_form("x", "", "wip");
        form.tags = "Urgent".into();
        let create = |form: &TaskForm| {
            let (task, valid) = form.apply(template());
            valid.and_then(|()| form.insert(&db, &task))
        };
        let id = create(&form).expect("created");
        assert_eq!(db.tags_for_task(id).expect("tags").len(), 1);
        form.name = "y".into();
        form.github_issue = "abc".into();
        assert!(create(&form).is_err());
        form.github_issue = String::new();
        form.tags = "Nope".into();
        assert!(create(&form).is_err());
        assert_eq!(db.tasks_for_project(project).expect("rows").len(), 1);
    }

    #[test]
    fn task_form_keeps_the_project_and_normalizes_newlines() {
        let (t, valid) = task_form(" x ", "12", "wip").apply(task());
        assert!(valid.is_ok());
        assert_eq!((t.project_id, t.name.as_str()), (7, "x"));
        assert_eq!((t.github_issue, t.status), (Some(12), TaskStatus::Wip));
        assert_eq!(t.description, "a\nb");
    }

    #[test]
    fn task_form_rejects_bad_input_but_keeps_what_was_typed() {
        let bad = |name, issue, status| task_form(name, issue, status).apply(task());
        assert!(matches!(
            bad(" ", "", "wip").1,
            Err(IterError::EmptyName("task"))
        ));
        assert!(bad("x", "abc", "wip").1.is_err());
        let (t, valid) = bad("x", "", "nope");
        assert!(matches!(valid, Err(IterError::InvalidStatus(_))));
        assert_eq!((t.name.as_str(), t.status), ("x", TaskStatus::Queue));
        assert_eq!(bad("x", "", "done").0.github_issue, None);
    }

    #[test]
    fn task_form_sets_flags_schedule_and_tags() {
        let mut form = task_form("x", "", "queue");
        form.start_time = "2026-09-28 09:30".into();
        form.duration = "01:45".into();
        form.tags = " Urgent, ,work ".into();
        let (t, valid) = form.apply(task());
        assert!(valid.is_ok());
        assert_eq!(t.duration, Some(Duration(105)));
        assert!(t.start_time.is_some());
        assert_eq!(names(&form.tags), ["Urgent", "work"]);

        form.duration = "1h".into();
        assert!(form.apply(task()).1.is_err());
        form.duration = String::new();
        form.start_time = "tomorrow".into();
        assert!(form.apply(task()).1.is_err());
    }
}
