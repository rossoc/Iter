//! What the edit forms submit, and how it lands on a model.
//!
//! Each form is applied to the *existing* row rather than building a fresh
//! one, so everything the form doesn't show (`project_id`,
//! `organization_id`, ids) is carried through untouched -- the same rule
//! the YAML editor follows in `commands`.

use crate::error::{IterError, Result};
use crate::models::{Named, Settings};
use serde::Deserialize;

mod board;
mod org;
mod project;
mod task;

pub use board::BoardForm;
pub use org::OrgForm;
pub use project::ProjectForm;
pub use task::TaskForm;

/// The names of the controls every edit form has (also the names of the
/// submitted fields), shared by the forms' [`EditForm::field`] and the
/// pages (`edit_page.rs`).
pub const NAME: &str = "name";
pub const DESCRIPTION: &str = "description";

/// A checkbox is present in the submission (as `on`) only when ticked.
fn checked(value: &Option<String>) -> bool {
    value.is_some()
}

/// Browsers submit textareas with CRLF line endings; the database (and the
/// YAML editor) use LF.
pub fn text(value: &str) -> String {
    value.replace("\r\n", "\n")
}

/// The fields of a submission read as `(name, value)` pairs, for a form a
/// struct can't hold (repeated fields, one per ticked box).
struct Pairs<'a>(&'a [(String, String)]);

impl Pairs<'_> {
    /// The first value submitted for `key`, or "".
    fn first(&self, key: &str) -> String {
        self.get(key).unwrap_or_default().to_string()
    }

    /// The first value submitted for `key`, if any (a ticked checkbox).
    fn get(&self, key: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// The ids submitted under `key` (repeated, one per ticked box), once
    /// each and in order; what isn't an id is dropped.
    fn ids(&self, key: &str) -> Vec<i64> {
        let mut ids: Vec<i64> = self
            .0
            .iter()
            .filter(|(k, _)| k == key)
            .filter_map(|(_, v)| v.parse().ok())
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }
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
    /// The names of its controls (also the names of the submitted fields),
    /// used by `settings_fields.rs`.
    pub const GITHUB: &'static str = "github";
    pub const TMUX: &'static str = "tmux";
    pub const AUTO_BRANCH: &'static str = "auto_branch";
    pub const BRANCH_TEMPLATE: &'static str = "branch_template";
    pub const DEFAULT_BRANCH: &'static str = "default_branch";
    pub const GITHUB_PROJECT: &'static str = "github_project";

    fn parse(pairs: &Pairs) -> SettingsForm {
        let on = |key| pairs.get(key).map(str::to_string);
        SettingsForm {
            github: on(Self::GITHUB),
            tmux: on(Self::TMUX),
            auto_branch: on(Self::AUTO_BRANCH),
            branch_template: pairs.first(Self::BRANCH_TEMPLATE),
            default_branch: pairs.first(Self::DEFAULT_BRANCH),
            github_project: pairs.first(Self::GITHUB_PROJECT),
        }
    }

    pub fn settings(&self) -> Settings {
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

/// The name of the checkboxes of a group's projects (also the name of the
/// submitted field), shared by [`OrgForm`], [`BoardForm`] and their pages.
pub const PROJECTS: &str = "project";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Organization, Project, Task, TaskStatus};
    use crate::web::edit::EditForm;

    /// The errors of the name belong to the Name field on every form, and
    /// the others to none.
    #[test]
    fn name_errors_belong_to_the_name_field_by_default() {
        assert_eq!(
            OrgForm::field(&IterError::EmptyName("organization")),
            Some(NAME)
        );
        assert_eq!(
            OrgForm::field(&IterError::CommaInName {
                kind: "organization",
                name: "a,b".into()
            }),
            Some(NAME)
        );
        assert_eq!(OrgForm::field(&IterError::CommandFailed("x".into())), None);
    }

    /// An organization or a task whose name is taken is refused on its Name
    /// field, in words.
    #[test]
    fn taken_organization_and_task_names_read_as_sentences() {
        let db = crate::db::Db::open(":memory:").expect("db");
        let org = Organization {
            name: "acme".into(),
            ..Organization::default()
        };
        db.insert(&org).expect("first");
        let taken = db.insert(&org).expect_err("same name");
        assert_eq!(OrgForm::field(&taken), Some(NAME));
        assert_eq!(
            OrgForm::refusal(&taken).message,
            "An organization with this name already exists."
        );
        let mut p = Project::template(&Settings::default());
        p.name = "p".into();
        p.base_path = "/tmp/p".into();
        let project = db.insert(&p).expect("project");
        let mut task = Task::template(project, String::new(), TaskStatus::Queue);
        task.name = "t".into();
        db.insert(&task).expect("first task");
        let taken = db.insert(&task).expect_err("same name");
        assert_eq!(TaskForm::field(&taken), Some(NAME));
        assert_eq!(
            TaskForm::refusal(&taken).message,
            "A task with this name already exists in this project."
        );
    }

    #[test]
    fn joined_is_the_inverse_of_names() {
        let tags = crate::models::Tag::defaults();
        assert_eq!(joined(&tags), "Urgent, Important");
        assert_eq!(names(&joined(&tags)), ["Urgent", "Important"]);
    }
}
