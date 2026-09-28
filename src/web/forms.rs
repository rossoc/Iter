//! What the edit forms submit, and how it lands on a model.
//!
//! Each form is applied to the *existing* row rather than building a fresh
//! one, so everything the form doesn't show (`project_id`,
//! `organization_id`, ids) is carried through untouched -- the same rule
//! the YAML editor follows in `commands`.

use super::edit::EditForm;
use crate::db::Db;
use crate::error::{IterError, Result};
use crate::models::{
    Configured, Duration, Named, Organization, Project, Settings, Tag, Task, TaskStatus,
    parse_start_time, require_name,
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

/// The six [`Settings`] fields as a form submits them -- flattened into
/// both the organization and the project form.
#[derive(Deserialize)]
pub struct SettingsForm {
    pub github: Option<String>,
    pub tmux: Option<String>,
    pub auto_branch: Option<String>,
    pub branch_template: String,
    pub default_branch: String,
    pub github_project: String,
}

impl SettingsForm {
    fn settings(&self) -> Settings {
        Settings {
            github: checked(&self.github),
            tmux: checked(&self.tmux),
            auto_branch: checked(&self.auto_branch),
            branch_template: self.branch_template.trim().to_string(),
            default_branch: self.default_branch.trim().to_string(),
            github_project: self.github_project.trim().to_string(),
        }
    }
}

#[derive(Deserialize)]
pub struct OrgForm {
    pub name: String,
    pub description: String,
    #[serde(flatten)]
    pub settings: SettingsForm,
    /// Project names, comma-separated.
    pub projects: String,
}

impl EditForm for OrgForm {
    type Row = Organization;

    fn saved(id: i64) -> String {
        format!("/org/{id}")
    }

    fn apply(&self, mut org: Organization) -> (Organization, Result<()>) {
        org.name = self.name.trim().to_string();
        org.description = text(&self.description);
        org.set_settings(self.settings.settings());
        let valid = org.validate();
        (org, valid)
    }

    /// The roster too. Resolved before anything is written, so an unknown
    /// name leaves the organization untouched.
    fn save(&self, db: &Db, id: i64, org: &Organization) -> Result<()> {
        let project_ids = db.ids_by_name::<Project>(&names(&self.projects))?;
        db.update(id, org)?;
        db.set_projects::<Organization>(id, &project_ids)
    }
}

#[derive(Deserialize)]
pub struct ProjectForm {
    pub name: String,
    pub description: String,
    pub base_path: String,
    /// The organization's id, or empty for none.
    pub organization: String,
    #[serde(flatten)]
    pub settings: SettingsForm,
}

impl EditForm for ProjectForm {
    type Row = Project;

    fn saved(id: i64) -> String {
        format!("/project/{id}")
    }

    fn apply(&self, mut project: Project) -> (Project, Result<()>) {
        project.name = self.name.trim().to_string();
        project.description = text(&self.description);
        project.base_path = self.base_path.trim().to_string();
        project.organization_id = self.organization.trim().parse().ok();
        project.set_settings(self.settings.settings());
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
    /// `yyyy-mm-dd hh:mm`, or empty for unscheduled.
    pub start_time: String,
    /// `hh:mm`, or empty for none.
    pub duration: String,
    /// Tag names, comma-separated.
    pub tags: String,
}

impl EditForm for TaskForm {
    type Row = Task;

    fn saved(id: i64) -> String {
        format!("/task/{id}")
    }

    /// A field that doesn't parse keeps the row's old value; the first such
    /// error is the one reported.
    fn apply(&self, mut task: Task) -> (Task, Result<()>) {
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
        task.name = self.name.trim().to_string();
        task.description = text(&self.description);
        task.branch_prefix = self.branch_prefix.trim().to_string();
        (task, valid)
    }

    /// The tags too, resolved first so an unknown name writes nothing.
    fn save(&self, db: &Db, id: i64, task: &Task) -> Result<()> {
        let tag_ids = db.ids_by_name::<Tag>(&names(&self.tags))?;
        db.update(id, task)?;
        db.set_task_tags(id, &tag_ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Settings;

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

    /// The settings arrive flattened among the form's own fields, decoded
    /// the way topcoat decodes a real submission: an unticked box is simply
    /// absent.
    #[test]
    fn a_submission_decodes_its_flattened_settings() {
        let body = "name=acme&description=&github=on&branch_template=fix%2F%7Btask%7D\
                    &default_branch=dev&github_project=Roadmap&projects=a%2C+b";
        let topcoat::router::content::Form(form) =
            topcoat::router::content::Form::<OrgForm>::from_bytes(body.as_bytes())
                .expect("decodes");
        let (org, valid) = form.apply(Organization::default());
        assert!(valid.is_ok());
        assert_eq!(
            org.settings(),
            Settings {
                github: true,
                tmux: false,
                auto_branch: false,
                branch_template: "fix/{task}".into(),
                default_branch: "dev".into(),
                github_project: "Roadmap".into(),
            }
        );
        assert_eq!(names(&form.projects), ["a", "b"]);
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

    #[test]
    fn project_form_moves_between_organizations_and_requires_a_path() {
        let form = |org: &str, path: &str| ProjectForm {
            name: "p".into(),
            description: String::new(),
            base_path: path.into(),
            organization: org.into(),
            settings: SettingsForm {
                github: Some("on".into()),
                tmux: None,
                auto_branch: None,
                branch_template: "feat/".into(),
                default_branch: "main".into(),
                github_project: String::new(),
            },
        };
        let base = Project::template(&Settings::default());
        let apply = |org: &str, path: &str| form(org, path).apply(base.clone());
        let (p, valid) = apply("3", "/tmp/p");
        assert!(valid.is_ok());
        assert_eq!(p.organization_id, Some(3));
        assert!(p.github && !p.tmux);
        assert_eq!(apply("", "/tmp/p").0.organization_id, None);
        assert!(matches!(apply("", " ").1, Err(IterError::EmptyBasePath)));
    }
}
