//! What the edit forms submit, and how it lands on a model.

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

pub const NAME: &str = "name";
pub const DESCRIPTION: &str = "description";

/// Present (as `on`) only when ticked: for `deserialize_with = "ticked"` on a `bool`.
fn ticked<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<bool, D::Error> {
    Option::<String>::deserialize(deserializer).map(|value| value.is_some())
}

/// Browsers submit textareas with CRLF line endings; the database (and the
/// YAML editor) use LF.
pub fn text(value: &str) -> String {
    value.replace("\r\n", "\n")
}

struct Pairs<'a>(&'a [(String, String)]);

impl Pairs<'_> {
    fn first(&self, key: &str) -> String {
        self.get(key).unwrap_or_default().to_string()
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

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

pub fn names(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .collect()
}

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
pub struct SettingsForm {
    #[serde(default, deserialize_with = "ticked")]
    pub github: bool,
    #[serde(default, deserialize_with = "ticked")]
    pub tmux: bool,
    #[serde(default, deserialize_with = "ticked")]
    pub auto_branch: bool,
    pub branch_template: String,
    pub default_branch: String,
    pub github_project: String,
}

impl SettingsForm {
    pub const GITHUB: &'static str = "github";
    pub const TMUX: &'static str = "tmux";
    pub const AUTO_BRANCH: &'static str = "auto_branch";
    pub const BRANCH_TEMPLATE: &'static str = "branch_template";
    pub const DEFAULT_BRANCH: &'static str = "default_branch";
    pub const GITHUB_PROJECT: &'static str = "github_project";

    fn parse(pairs: &Pairs) -> SettingsForm {
        let on = |key| pairs.get(key).is_some();
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
            github: self.github,
            tmux: self.tmux,
            auto_branch: self.auto_branch,
            branch_template: self.branch_template.trim().to_string(),
            default_branch: self.default_branch.trim().to_string(),
            github_project: self.github_project.trim().to_string(),
        }
    }
}

pub const PROJECTS: &str = "project";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::models::{Organization, Task, TaskStatus};
    use crate::web::edit::EditForm;

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

    #[test]
    fn taken_organization_and_task_names_read_as_sentences() {
        let db = fixtures::db();
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
        let project = fixtures::add_project(&db, "p");
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
