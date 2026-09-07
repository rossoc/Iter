use crate::models::{default_branch_template, default_true};
use iter_macros::Table;
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
#[derive(Debug, Clone, Serialize, Deserialize, Table)]
#[table(name = "organizations", order_by = "name")]
pub struct Organization {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub id: Option<i64>,

    pub name: String,

    /// Free-form markdown notes. Edited below the `---` separator in the
    /// YAML editor rather than as a field among the others -- see
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
}

impl Organization {
    /// A blank template for `iter organization new` to open in the YAML
    /// editor. Its defaults are the same ones a project falls back to when
    /// it belongs to no organization at all.
    pub fn template() -> Self {
        Organization {
            id: None,
            name: String::new(),
            description: String::new(),
            github: false,
            tmux: true,
            auto_branch: true,
            branch_template: default_branch_template(),
        }
    }
}

impl crate::models::MarkdownBody for Organization {
    fn description(&self) -> &str {
        &self.description
    }

    fn set_description(&mut self, description: String) {
        self.description = description;
    }
}
