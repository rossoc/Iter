use crate::db::Table;
use crate::error::{IterError, Result};
use serde::Serialize;
use serde::de::DeserializeOwned;

mod board;
mod organization;
mod project;
mod session;
mod session_config;
mod tag;
mod task;

pub use board::Board;
pub use organization::{Organization, OrganizationEdit};
pub use project::Project;
pub use session::Session;
pub use session_config::SessionConfig;
pub use tag::Tag;
#[cfg(feature = "web")]
pub use tag::{IMPORTANT_TAG, URGENT_TAG};
pub use task::{Duration, START_TIME_FMT, Task, TaskStatus};

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

/// Everything the editor round trip needs of a type: written out as YAML,
/// read back from it, with the description as the markdown body. Blanket
/// implemented, so it's only ever a shorthand for the three bounds.
pub trait Editable: Serialize + DeserializeOwned + MarkdownBody {}

impl<T: Serialize + DeserializeOwned + MarkdownBody> Editable for T {}

/// A stored type whose `name` is unique across the whole database -- what
/// the CLI addresses it by, what `Db::resolve` looks it up by, what shell
/// completion offers and what `iter <entity> list` prints. Everything that
/// is "the same for every named entity" hangs off this one trait instead of
/// being written once per entity.
///
/// Deliberately not implemented for [`Task`]: a task's name is unique only
/// within its project (`UNIQUE (project_id, name)`), so a bare task name
/// addresses nothing on its own. Tasks are addressed by [`task_ref`]
/// instead, and the two task listings that do exist -- `Db::task_refs` and
/// `Db::task_names` -- are their own queries precisely because neither is
/// "every row's name".
pub(crate) trait Named: Table {
    /// What messages call one of these: "board", "tag", ...
    const KIND: &'static str;

    fn name(&self) -> &str;

    /// Normalises and checks an edited item before it's written. By default
    /// only that it has a name.
    fn validate(&mut self) -> Result<()> {
        require_name(Self::KIND, self.name())
    }
}

/// Implements [`Named`] for a type with a `name: String` field.
macro_rules! named {
    ($ty:ty, $kind:literal) => {
        impl crate::models::Named for $ty {
            const KIND: &'static str = $kind;

            fn name(&self) -> &str {
                &self.name
            }
        }
    };
}
pub(crate) use named;

/// A named container of projects -- a board or an organization. A project
/// belongs to at most one of each, through a nullable foreign key on
/// `projects` that is `ON DELETE SET NULL`, so deleting the container only
/// detaches its projects.
pub(crate) trait ProjectGroup: Named {
    /// The `projects` column holding membership.
    const MEMBER_COLUMN: &'static str;

    /// What a project in none of these is: "isn't bound to a board".
    const NOT_A_MEMBER: &'static str;

    /// Where a project is once its container is gone: "on no board".
    const DETACHED: &'static str;

    /// The id of the container `project` is in, if any.
    fn group_of(project: &Project) -> Option<i64>;
}

/// The one "must have a name" check, for named entities and tasks alike.
pub(crate) fn require_name(kind: &'static str, name: &str) -> Result<()> {
    match name.trim().is_empty() {
        true => Err(IterError::EmptyName(kind)),
        false => Ok(()),
    }
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
