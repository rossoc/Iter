use crate::models::Named;
use crate::models::{
    Configured, Organization, Settings, default_branch_template, default_true, main_branch,
    require_no_comma,
};
use iter_macros::{MarkdownBody, Table};
use serde::{Deserialize, Serialize};

/// A project: a base directory of work, optionally backed by a git repo,
/// with its own tasks. `id` is `None` for a not-yet-created project (the
/// blank template opened in the editor); it's filled in once inserted.
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

    /// The organization this project belongs to, or `None` -- membership is
    /// optional, and a project without one behaves exactly as it did before
    /// organizations existed, falling back to its own stored settings.
    ///
    /// Not part of the editor's view: it's set from `--organization`
    /// and otherwise carried through untouched, the same way a task's
    /// `project_id` is.
    #[serde(skip)]
    pub organization_id: Option<i64>,

    /// The board this project is bound to, or `None`. A board is a
    /// container of projects, so a project sits on at most one.
    ///
    /// Not part of the editor's view, for the same reason as
    /// `organization_id`: it's set from `--board`.
    #[serde(skip)]
    pub board_id: Option<i64>,

    pub name: String,

    /// Free-form markdown notes. Edited as the markdown body below the
    /// YAML front matter rather than as a field among the others -- see
    /// `MarkdownBody`.
    #[serde(skip)]
    pub description: String,

    pub base_path: String,

    /// Whether `base_path` is a git/GitHub repo -- gates worktree/branch
    /// creation and `gh` issue integration for this project's tasks.
    #[serde(default)]
    pub github: bool,

    /// Whether the tmux integration (session creation + hooks) is active
    /// for this project. When `false`, `iter session new` still tracks a
    /// session-config row (and worktree/branch, if `github`) but doesn't
    /// spawn an actual tmux session -- sessions are started/stopped
    /// manually via `iter session start`/`iter session stop` instead of
    /// tmux attach/detach.
    #[serde(default = "default_true")]
    pub tmux: bool,

    /// Whether `iter session new` creates a branch/worktree automatically.
    /// Only relevant when `github` is true.
    #[serde(default = "default_true")]
    pub auto_branch: bool,

    /// Branch name template; `{task}` is replaced with the slugified task name.
    #[serde(default = "default_branch_template")]
    pub branch_template: String,

    /// The branch finished work lands on: what `iter task done --save`
    /// merges into the task's worktree first, and then fast-forwards onto.
    /// `main` unless the project's trunk is called something else (`dev`,
    /// `master`, ...). Only relevant when the project is `github`-backed.
    #[serde(default = "main_branch")]
    pub default_branch: String,

    /// The GitHub Project (the board, not an `iter` project) that
    /// `iter task push` files a newly opened issue under, by its title --
    /// what `gh issue create --project` takes. Empty means "don't file it
    /// anywhere", which is what every project starts as.
    ///
    /// Only ever passed to `gh`, never checked here: `gh` resolves the
    /// title against the repo owner's projects and refuses to create the
    /// issue if there's no such board, so a wrong name costs a failed push
    /// rather than an issue filed in the wrong place. Note that reaching
    /// projects at all needs a token with the Projects permission
    /// (`gh auth refresh -s project` on a classic login).
    #[serde(default)]
    pub github_project: String,
}

impl Project {
    /// A blank template for `iter project new` to open in the editor,
    /// carrying the config file's project defaults -- which is where a
    /// user who always wants, say, `github: true` sets it once instead of
    /// flipping it in every buffer.
    pub fn template(defaults: &Settings) -> Self {
        let mut project = Project::default();
        project.set_settings(defaults.clone());
        project
    }

    /// Beyond a name: no comma in it (see [`require_no_comma`]), and a
    /// `base_path` -- a project is a place on disk.
    ///
    /// The path is expanded (`~`) and absolutised here, once, for every way
    /// a project is written -- created, edited in the editor or in the
    /// browser. A relative `base_path` names a different directory from
    /// every place `iter` is later run: a different worktree to create, a
    /// different repo to ask whether it's one.
    fn check(&mut self) -> crate::error::Result<()> {
        require_no_comma(Self::KIND, &self.name)?;
        let base_path = self.base_path.trim();
        if base_path.is_empty() {
            return Err(crate::error::IterError::EmptyBasePath);
        }
        self.base_path = crate::scaffold::absolute_path(base_path)?;
        Ok(())
    }

    /// Seeds this template with `organization`'s downstream defaults and
    /// makes it a member -- an organization's settings outrank the config
    /// file's, being the narrower answer to the same question. Only
    /// meaningful on a *blank* template: cloning an existing project keeps
    /// that project's own settings, and sets nothing here but the
    /// membership.
    pub fn inherit_from(&mut self, organization: &Organization) {
        self.organization_id = organization.id;
        self.set_settings(organization.settings());
    }
}
