//! What the edit forms submit, and how it lands on a model.
//!
//! Each form is applied to the *existing* row rather than building a fresh
//! one, so everything the form doesn't show (`project_id`,
//! `organization_id`, ids) is carried through untouched -- the same rule
//! the YAML editor follows in `commands`.

use crate::error::{IterError, Result};
use crate::models::{
    Duration, Named, Organization, Project, Task, TaskStatus, parse_start_time, require_name,
};
use serde::Deserialize;

/// A checkbox is present in the submission (as `on`) only when ticked.
fn checked(value: &Option<String>) -> bool {
    value.is_some()
}

/// Browsers submit textareas with CRLF line endings; the database (and the
/// YAML editor) use LF.
pub fn text(value: &str) -> String {
    value.replace("\r\n", "\n")
}

/// A comma-separated list of names, trimmed and without blanks.
pub fn names(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .collect()
}

/// The inverse of [`names`]: `items`' names, comma-separated.
pub fn joined<T: Named>(items: &[T]) -> String {
    items.iter().map(Named::name).collect::<Vec<_>>().join(", ")
}

/// Sets an optional field from its form text: blank clears it, a value that
/// `parse`s sets it, and anything else leaves it be and is the error.
fn optional<T>(
    slot: &mut Option<T>,
    raw: &str,
    parse: impl FnOnce(&str) -> Option<T>,
    error: impl FnOnce(&str) -> IterError,
) -> Result<()> {
    let raw = raw.trim();
    if raw.is_empty() {
        *slot = None;
    } else {
        *slot = Some(parse(raw).ok_or_else(|| error(raw))?);
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct OrgForm {
    pub name: String,
    pub description: String,
    pub github: Option<String>,
    pub tmux: Option<String>,
    pub auto_branch: Option<String>,
    pub branch_template: String,
    pub default_branch: String,
    pub github_project: String,
    /// Project names, comma-separated.
    pub projects: String,
}

impl OrgForm {
    /// `org` with the submission applied, and whether that submission is
    /// acceptable (see `Named::validate`). The row is filled in even when it
    /// isn't, so the form can be shown again with what the user typed.
    pub fn apply(&self, mut org: Organization) -> (Organization, Result<()>) {
        org.name = self.name.trim().to_string();
        org.description = text(&self.description);
        org.github = checked(&self.github);
        org.tmux = checked(&self.tmux);
        org.auto_branch = checked(&self.auto_branch);
        org.branch_template = self.branch_template.trim().to_string();
        org.default_branch = self.default_branch.trim().to_string();
        org.github_project = self.github_project.trim().to_string();
        let valid = org.validate();
        (org, valid)
    }
}

#[derive(Deserialize)]
pub struct ProjectForm {
    pub name: String,
    pub description: String,
    pub base_path: String,
    /// The organization's id, or empty for none.
    pub organization: String,
    pub github: Option<String>,
    pub tmux: Option<String>,
    pub auto_branch: Option<String>,
    pub branch_template: String,
    pub default_branch: String,
    pub github_project: String,
}

impl ProjectForm {
    /// See [`OrgForm::apply`].
    pub fn apply(&self, mut project: Project) -> (Project, Result<()>) {
        project.name = self.name.trim().to_string();
        project.description = text(&self.description);
        project.base_path = self.base_path.trim().to_string();
        project.organization_id = self.organization.trim().parse().ok();
        project.github = checked(&self.github);
        project.tmux = checked(&self.tmux);
        project.auto_branch = checked(&self.auto_branch);
        project.branch_template = self.branch_template.trim().to_string();
        project.default_branch = self.default_branch.trim().to_string();
        project.github_project = self.github_project.trim().to_string();
        let valid = project.validate();
        (project, valid)
    }
}

#[derive(Deserialize)]
pub struct TaskForm {
    pub name: String,
    pub description: String,
    /// The issue number, or empty for none.
    pub github_issue: String,
    pub status: String,
    pub branch_prefix: String,
    pub urgency: Option<String>,
    pub importance: Option<String>,
    /// `yyyy-mm-dd hh:mm`, or empty for unscheduled.
    pub start_time: String,
    /// `hh:mm`, or empty for none.
    pub duration: String,
    /// Tag names, comma-separated.
    pub tags: String,
}

impl TaskForm {
    /// See [`OrgForm::apply`]. A field that doesn't parse keeps the row's old
    /// value; the first such error is the one reported.
    pub fn apply(&self, mut task: Task) -> (Task, Result<()>) {
        let mut valid = require_name("task", &self.name);
        valid = valid.and(optional(
            &mut task.github_issue,
            &self.github_issue,
            |s| s.parse().ok(),
            |raw| IterError::CommandFailed(format!("invalid github issue number '{raw}'")),
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
                    "invalid start time '{raw}', expected yyyy-mm-dd hh:mm"
                ))
            },
        ));
        valid = valid.and(optional(
            &mut task.duration,
            &self.duration,
            Duration::parse,
            |raw| IterError::CommandFailed(format!("invalid duration '{raw}', expected hh:mm")),
        ));
        task.urgency = checked(&self.urgency);
        task.importance = checked(&self.importance);
        task.name = self.name.trim().to_string();
        task.description = text(&self.description);
        task.branch_prefix = self.branch_prefix.trim().to_string();
        (task, valid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ProjectDefaults;

    fn task_form(name: &str, issue: &str, status: &str) -> TaskForm {
        TaskForm {
            name: name.into(),
            description: "a\r\nb".into(),
            github_issue: issue.into(),
            status: status.into(),
            branch_prefix: "feat/".into(),
            urgency: None,
            importance: None,
            start_time: String::new(),
            duration: String::new(),
            tags: String::new(),
        }
    }

    fn task() -> Task {
        Task::template(7, String::new(), TaskStatus::Queue)
    }

    #[test]
    fn joined_is_the_inverse_of_names() {
        let tags = crate::models::Tag::defaults();
        assert_eq!(joined(&tags), "Urgent, Important");
        assert_eq!(names(&joined(&tags)), ["Urgent", "Important"]);
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
        form.urgency = Some("on".into());
        form.start_time = "2026-09-28 09:30".into();
        form.duration = "01:45".into();
        form.tags = " Urgent, ,work ".into();
        let (t, valid) = form.apply(task());
        assert!(valid.is_ok());
        assert!(t.urgency && !t.importance);
        assert_eq!(t.duration, Some(Duration(105)));
        assert!(t.start_time.is_some());
        assert_eq!(names(&form.tags), ["Urgent", "work"]);

        form.duration = "1h".into();
        assert!(form.apply(task()).1.is_err());
        form.duration = String::new();
        form.start_time = "tomorrow".into();
        assert!(form.apply(task()).1.is_err());
    }

    #[test]
    fn project_form_moves_between_organizations_and_requires_a_path() {
        let form = |org: &str, path: &str| ProjectForm {
            name: "p".into(),
            description: String::new(),
            base_path: path.into(),
            organization: org.into(),
            github: Some("on".into()),
            tmux: None,
            auto_branch: None,
            branch_template: "feat/".into(),
            default_branch: "main".into(),
            github_project: String::new(),
        };
        let base = Project::template(&ProjectDefaults::default());
        let apply = |org: &str, path: &str| form(org, path).apply(base.clone());
        let (p, valid) = apply("3", "/tmp/p");
        assert!(valid.is_ok());
        assert_eq!(p.organization_id, Some(3));
        assert!(p.github && !p.tmux);
        assert_eq!(apply("", "/tmp/p").0.organization_id, None);
        assert!(matches!(apply("", " ").1, Err(IterError::EmptyBasePath)));
    }
}
