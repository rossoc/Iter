use iter_macros::Table;
use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

fn default_branch_template() -> String {
    "feat/{task}".to_string()
}

/// A project: a base directory of work, optionally backed by a git repo,
/// with its own tasks. `id` is `None` for a not-yet-created project (the
/// blank template opened in the YAML editor); it's filled in once inserted.
#[derive(Debug, Clone, Serialize, Deserialize, Table)]
#[table(name = "projects", order_by = "name")]
pub struct Project {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub id: Option<i64>,

    pub name: String,

    /// Free-form markdown notes. Edited below the `---` separator in the
    /// YAML editor rather than as a field among the others -- see
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
}

impl Project {
    /// A blank template for `iter project new` to open in the YAML editor.
    pub fn template() -> Self {
        Project {
            id: None,
            name: String::new(),
            description: String::new(),
            base_path: String::new(),
            github: false,
            tmux: true,
            auto_branch: true,
            branch_template: default_branch_template(),
        }
    }
}

impl crate::models::MarkdownBody for Project {
    fn description(&self) -> &str {
        &self.description
    }

    fn set_description(&mut self, description: String) {
        self.description = description;
    }
}
