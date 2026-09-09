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

/// The value a project's `default_branch` -- the branch `iter task done
/// --save` merges finished work into -- starts as. Git's own out-of-the-box
/// name for it; a project whose trunk is `dev` (or anything else) says so
/// in its own front matter.
pub(crate) fn main_branch() -> String {
    "main".to_string()
}

/// A type edited via `md_edit::edit_in_editor` that carries a free-form
/// markdown `description`. The field is `#[serde(skip)]`ed on the type
/// itself and instead placed below the YAML front matter's closing `---`,
/// as the markdown body
pub trait MarkdownBody {
    fn description(&self) -> &str;
    fn set_description(&mut self, description: String);
}

/// A type whose `name` is unique across the whole database -- what the CLI
/// addresses it by, what `Db::find_by_name` looks it up by, what shell
/// completion offers and what `iter <entity> list` prints. One method is
/// enough to give every such type the same generic lookup, listing and
/// completion instead of one hand-written copy per entity.
///
/// Deliberately not implemented for [`Task`]: a task's name is unique only
/// within its project (`UNIQUE (project_id, name)`), so a bare task name
/// addresses nothing on its own. Tasks are addressed by [`task_ref`]
/// instead, and the two task listings that do exist -- `Db::task_refs` and
/// `Db::task_names` -- are their own queries precisely because neither is
/// "every row's name".
pub trait Named {
    fn name(&self) -> &str;
}

/// How a task is named wherever the CLI speaks about one: `<project>/<task>`.
/// The separator lives here, next to [`split_task_ref`] which reads it back,
/// so the two halves of the convention can't drift apart.
pub fn task_ref(project: &str, task: &str) -> String {
    format!("{project}/{task}")
}

/// Splits a [`task_ref`] on its first separator.
pub fn split_task_ref(task_ref: &str) -> Option<(&str, &str)> {
    task_ref.split_once('/')
}
