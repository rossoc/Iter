use crate::db::Table;
use crate::error::{IterError, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

mod board;
mod organization;
mod project;
mod push;
mod session;
mod session_config;
mod settings;
mod tag;
mod task;

pub use board::{Board, Card};
pub use organization::Organization;
pub use project::Project;
pub use push::Push;
pub use session::Session;
pub use session_config::SessionConfig;
pub(crate) use settings::Configured;
pub use settings::Settings;
#[cfg(feature = "web")]
pub use tag::is_hex_color;
pub use tag::{IMPORTANT_TAG, Priority, Tag, URGENT_TAG};
#[cfg(feature = "web")]
pub use task::parse_start_time;
pub use task::{Duration, START_TIME_FMT, Task, TaskEdit, TaskStatus, end_text};

/// Fallback settings for a project that inherits none (no organization); also serde's default
/// for fields the YAML editor left out.
pub(crate) fn default_true() -> bool {
    true
}

pub(crate) fn default_branch_template() -> String {
    "feat/{task}".to_string()
}

/// A project's default `default_branch`: git's own default.
pub(crate) fn main_branch() -> String {
    "main".to_string()
}

/// An edited type with a free-form markdown `description`, `#[serde(skip)]`ed and placed below
/// the YAML front matter.
pub trait MarkdownBody {
    fn description(&self) -> &str;
    fn set_description(&mut self, description: String);

    /// Copies from `original` every field the buffer doesn't carry, so an edit can't lose them.
    fn carry_over(&mut self, _original: &Self) {}

    /// Reads the buffer's front matter; edit types wrapping a row plus a list override it
    /// ([`read_wrapped`]).
    fn from_front_matter(front: &str) -> serde_yaml::Result<Self>
    where
        Self: DeserializeOwned,
    {
        serde_yaml::from_str(front)
    }
}

/// Reads front matter of a row plus one list as two plain reads, not `#[serde(flatten)]`, which
/// buffers values and breaks bare scalars (`name: 2025`, `default_branch: 1.10` become numbers).
/// Each read ignores the other's key.
pub(crate) fn read_wrapped<R: DeserializeOwned>(
    front: &str,
    list: &str,
) -> serde_yaml::Result<(R, Vec<String>)> {
    let row = serde_yaml::from_str(front)?;
    let mut mapping: serde_yaml::Mapping = serde_yaml::from_str(front)?;
    let names = match mapping.remove(list) {
        Some(value) if !value.is_null() => serde_yaml::from_value(value)?,
        _ => Vec::new(),
    };
    Ok((row, names))
}

/// Everything the editor round trip needs: YAML out and in, description as markdown body.
/// Blanket implemented.
pub trait Editable: Serialize + DeserializeOwned + MarkdownBody {}

impl<T: Serialize + DeserializeOwned + MarkdownBody> Editable for T {}

/// A stored type with a database-wide unique `name`: how the CLI addresses it, what completion
/// offers and `list` prints. Implemented by `#[derive(Table)]` given a `kind`. Not implemented for
/// [`Task`]: its name is unique only within its project, so tasks are addressed by [`task_ref`]
/// (`Db::task_refs`/`Db::task_names` are their own queries).
pub(crate) trait Named: Table {
    /// What messages call one of these: "board", "tag", ...
    const KIND: &'static str;

    fn name(&self) -> &str;

    /// Normalises and checks an edited item before it's written; by default only that it has a name.
    fn validate(&mut self) -> Result<()> {
        require_name(Self::KIND, self.name())
    }
}

/// A named container of projects (board or organization), through a nullable `ON DELETE SET NULL`
/// foreign key: deleting it only detaches its projects.
pub(crate) trait ProjectGroup: Named {
    const MEMBER_COLUMN: &'static str;

    const NOT_A_MEMBER: &'static str;

    const DETACHED: &'static str;

    fn group_of(project: &Project) -> Option<i64>;
}

/// What `board edit`/`organization edit` open: the group plus its project names. The roster is
/// each project's foreign key, so saving it moves projects in and out.
#[derive(Debug, Serialize, Deserialize)]
pub struct GroupEdit<G> {
    #[serde(flatten)]
    pub group: G,

    /// Project names: listed ones move here, omitted ones move out to none.
    #[serde(default)]
    pub projects: Vec<String>,
}

impl<G: MarkdownBody + DeserializeOwned> MarkdownBody for GroupEdit<G> {
    fn description(&self) -> &str {
        self.group.description()
    }

    fn set_description(&mut self, description: String) {
        self.group.set_description(description);
    }

    fn carry_over(&mut self, original: &Self) {
        self.group.carry_over(&original.group);
    }

    fn from_front_matter(front: &str) -> serde_yaml::Result<Self> {
        let (group, projects) = read_wrapped(front, "projects")?;
        Ok(GroupEdit { group, projects })
    }
}

pub(crate) fn require_name(kind: &'static str, name: &str) -> Result<()> {
    match name.trim().is_empty() {
        true => Err(IterError::EmptyName(kind)),
        false => Ok(()),
    }
}

/// Refuses a comma in names the web forms list comma-separated.
pub(crate) fn require_no_comma(kind: &'static str, name: &str) -> Result<()> {
    if name.contains(',') {
        return Err(IterError::CommaInName {
            kind,
            name: name.to_string(),
        });
    }
    Ok(())
}

/// How the CLI names a task: `<project>/<task>`. The separator lives beside [`split_task_ref`].
pub fn task_ref(project: &str, task: &str) -> String {
    format!("{project}/{task}")
}

pub fn split_task_ref(task_ref: &str) -> Option<(&str, &str)> {
    task_ref.split_once('/')
}
