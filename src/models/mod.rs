mod organization;
mod project;
mod session;
mod session_config;
mod task;

pub use organization::Organization;
pub use project::Project;
pub use session::Session;
pub use session_config::SessionConfig;
pub use task::{Task, TaskStatus};

/// The fallback settings a project uses when it inherits none -- i.e. when
/// it belongs to no organization. Shared by `Project` and `Organization`,
/// which default the same way, and used by serde to fill in a field the
/// YAML editor left out.
pub(crate) fn default_true() -> bool {
    true
}

pub(crate) fn default_branch_template() -> String {
    "feat/{task}".to_string()
}

/// A type edited via `md_edit::edit_in_nvim` that carries a free-form
/// markdown `description`. The field is `#[serde(skip)]`ed on the type
/// itself and instead placed below the YAML front matter's closing `---`,
/// as the markdown body
pub trait MarkdownBody {
    fn description(&self) -> &str;
    fn set_description(&mut self, description: String);
}
