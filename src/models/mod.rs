use crate::db::Table;
use crate::error::{IterError, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

mod board;
mod organization;
mod project;
mod session;
mod session_config;
mod settings;
mod tag;
mod task;

pub use board::{Board, Card};
pub use organization::Organization;
pub use project::Project;
pub use session::Session;
pub use session_config::SessionConfig;
pub(crate) use settings::Configured;
pub use settings::Settings;
pub use tag::{IMPORTANT_TAG, Priority, Tag, URGENT_TAG};
#[cfg(feature = "web")]
pub use task::parse_start_time;
pub use task::{Duration, START_TIME_FMT, Task, TaskEdit, TaskStatus};

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

    /// Copies from `original` -- the item the editor was opened on -- every
    /// field the buffer doesn't carry, so an edit can't lose them. The
    /// derive writes this for every other `#[serde(skip)]` field.
    fn carry_over(&mut self, _original: &Self) {}
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
/// being written once per entity. Implemented by `#[derive(Table)]` given a
/// `kind` -- see the derive for how a type adds rules to `validate`.
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

/// What `iter board edit`/`iter organization edit` open in the editor: the
/// group plus the names of the projects in it. The roster isn't a column --
/// it's each project's foreign key -- so it rides alongside the row rather
/// than on it, and saving it moves projects in and out.
#[derive(Debug, Serialize, Deserialize)]
pub struct GroupEdit<G> {
    #[serde(flatten)]
    pub group: G,

    /// Project names. Listing one that's in another group moves it here;
    /// leaving one off moves it out to none.
    #[serde(default)]
    pub projects: Vec<String>,
}

impl<G: MarkdownBody> MarkdownBody for GroupEdit<G> {
    fn description(&self) -> &str {
        self.group.description()
    }

    fn set_description(&mut self, description: String) {
        self.group.set_description(description);
    }

    fn carry_over(&mut self, original: &Self) {
        self.group.carry_over(&original.group);
    }
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
