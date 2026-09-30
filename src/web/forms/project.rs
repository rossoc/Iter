//! The project edit form.

use super::{SettingsForm, text};
use crate::error::{IterError, Result};
use crate::models::{Configured, Named, Project};
use crate::web::edit::{EditForm, name_field};
use crate::web::url::project_url;
use serde::Deserialize;

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

impl ProjectForm {
    /// The names of the controls an error can be about (also the names of
    /// the submitted fields), shared by [`EditForm::field`] and the page.
    pub const BASE_PATH: &'static str = "base_path";
    pub const ORGANIZATION: &'static str = "organization";
}

impl EditForm for ProjectForm {
    type Row = Project;
    fn saved(id: i64) -> String {
        project_url(id)
    }

    fn field(error: &IterError) -> Option<&'static str> {
        match error {
            IterError::EmptyBasePath => Some(Self::BASE_PATH),
            _ => name_field(error),
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Settings;
    use crate::web::forms::NAME;

    /// The project errors that belong to a field say which, and a taken
    /// name reads as a sentence, not as a database error.
    #[test]
    fn project_errors_name_their_field() {
        assert_eq!(
            ProjectForm::field(&IterError::EmptyBasePath),
            Some(ProjectForm::BASE_PATH)
        );
        assert_eq!(
            ProjectForm::field(&IterError::EmptyName("project")),
            Some(NAME)
        );
        let db = crate::db::Db::open(":memory:").expect("db");
        let mut p = Project::template(&Settings::default());
        p.name = "p".into();
        p.base_path = "/tmp/p".into();
        db.insert(&p).expect("first");
        let taken = db.insert(&p).expect_err("same name");
        assert_eq!(ProjectForm::field(&taken), Some(NAME));
        assert_eq!(
            ProjectForm::refusal(&taken).message,
            "A project with this name already exists."
        );
        let other = IterError::CommandFailed("x".into());
        assert_eq!(ProjectForm::field(&other), None);
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
