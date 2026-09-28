use crate::models::{Configured, Settings, default_branch_template, default_true, main_branch};
use iter_macros::{MarkdownBody, Table};
use serde::{Deserialize, Serialize};

/// A grouping of projects, and the defaults new projects in it start from.
///
/// Deliberately has no `base_path`: an organization isn't a place on disk,
/// it's the settings and the roster its projects share. Everything a
/// project needs that *is* about disk -- where it lives, whether that
/// directory is a repo -- stays on the project, which is why `iter
/// init`/`new`/`clone` still create projects rather than organizations.
///
/// `id` is `None` for a not-yet-created organization (the blank template
/// opened in the YAML editor); it's filled in once inserted.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Table, MarkdownBody)]
#[table(name = "organizations", order_by = "name", kind = "organization")]
pub struct Organization {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub id: Option<i64>,

    pub name: String,

    /// Free-form markdown notes. Edited as the markdown body below the
    /// YAML front matter rather than as a field among the others -- see
    /// `MarkdownBody`.
    #[serde(skip)]
    pub description: String,

    /// Default `github` for projects created in this organization.
    #[serde(default)]
    pub github: bool,

    /// Default `tmux` for projects created in this organization.
    #[serde(default = "default_true")]
    pub tmux: bool,

    /// Default `auto_branch` for projects created in this organization.
    #[serde(default = "default_true")]
    pub auto_branch: bool,

    /// Default `branch_template` for projects created in this organization.
    #[serde(default = "default_branch_template")]
    pub branch_template: String,

    /// Default `default_branch` for projects created in this organization.
    #[serde(default = "main_branch")]
    pub default_branch: String,

    /// Default `github_project` for projects created in this organization.
    #[serde(default)]
    pub github_project: String,
}

impl Organization {
    /// A blank template for `iter organization new` to open in the
    /// editor. Its fields *are* project defaults, so it starts from the
    /// same config block a project without an organization starts from.
    pub fn template(defaults: &Settings) -> Self {
        let mut organization = Organization::default();
        organization.set_settings(defaults.clone());
        organization
    }
}

impl crate::models::ProjectGroup for Organization {
    const MEMBER_COLUMN: &'static str = "organization_id";
    const NOT_A_MEMBER: &'static str = "doesn't belong to an organization";
    const DETACHED: &'static str = "without an organization";

    fn group_of(project: &crate::models::Project) -> Option<i64> {
        project.organization_id
    }
}
