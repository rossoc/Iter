use crate::models::Named;
use crate::models::{
    Configured, Organization, Settings, default_branch_template, default_true, main_branch,
    require_no_comma,
};
use iter_macros::{MarkdownBody, Table};
use serde::{Deserialize, Serialize};

/// A project: a base directory of work, optionally a git repo, with tasks.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Table, MarkdownBody)]
#[table(
    name = "projects",
    order_by = "name",
    kind = "project",
    check = "check"
)]
pub struct Project {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub id: Option<i64>,

    /// Set from `--organization`, not shown in the editor. A project without one uses its own
    /// stored settings.
    #[serde(skip)]
    pub organization_id: Option<i64>,

    /// Set from `--board`, not shown in the editor; at most one board.
    #[serde(skip)]
    pub board_id: Option<i64>,

    pub name: String,

    #[serde(skip)]
    pub description: String,

    pub base_path: String,

    /// Whether `base_path` is a git/GitHub repo: gates worktrees/branches and `gh` issues.
    #[serde(default)]
    pub github: bool,

    /// Whether tmux integration is on. When `false`, `session new` still tracks a config row (and
    /// worktree/branch if `github`) but spawns no tmux session.
    #[serde(default = "default_true")]
    pub tmux: bool,

    /// Whether `session new` creates a branch/worktree; needs `github`.
    #[serde(default = "default_true")]
    pub auto_branch: bool,

    /// Branch name template; `{task}` is replaced with the slugified task name.
    #[serde(default = "default_branch_template")]
    pub branch_template: String,

    /// The branch `task done --save` merges into; `main` unless the trunk is named otherwise.
    /// Needs `github`.
    #[serde(default = "main_branch")]
    pub default_branch: String,

    /// Title of the GitHub Project `iter task push` files new issues under; empty means none.
    /// Passed to `gh` unchecked: it fails the call on an unknown board rather than filing the issue
    /// elsewhere. Needs a token with the Projects permission (`gh auth refresh -s project`).
    #[serde(default)]
    pub github_project: String,

    /// Daily-notes folder (`~` expanded when read); empty means a folder named for the project
    /// inside its organization's `notes_path`.
    #[serde(default)]
    pub notes_path: String,
}

impl Project {
    /// A blank template for `iter project new`, carrying the config file's project defaults.
    pub fn template(defaults: &Settings) -> Self {
        let mut project = Project::default();
        project.set_settings(defaults.clone());
        project
    }

    /// Beyond a name: no comma ([`require_no_comma`]) and a `base_path`, expanded (`~`) and
    /// absolutised here for every write path: a relative path names a different directory wherever
    /// `iter` later runs.
    fn check(&mut self) -> crate::error::Result<()> {
        require_no_comma(Self::KIND, &self.name)?;
        if self.name.contains('/') {
            return Err(crate::error::IterError::SlashInName {
                kind: Self::KIND,
                name: self.name.clone(),
            });
        }
        let base_path = self.base_path.trim();
        if base_path.is_empty() {
            return Err(crate::error::IterError::EmptyBasePath);
        }
        self.base_path = crate::scaffold::absolute_path(base_path)?;
        Ok(())
    }

    /// Seeds a blank template with `organization`'s defaults (narrower than the config file's) and
    /// makes it a member. Cloning an existing project keeps its settings and sets only membership.
    pub fn inherit_from(&mut self, organization: &Organization) {
        self.organization_id = organization.id;
        self.set_settings(organization.settings());
    }
}
