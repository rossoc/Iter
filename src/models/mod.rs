use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

mod board;
mod organization;
mod project;
mod push;
mod session;
mod session_config;
mod settings;
pub(crate) mod sqltypes;
mod tag;
mod task;
mod task_ref;
mod traits;

pub use board::{Board, Card};
#[allow(unused_imports)] // stub: I3
pub use organization::{Organization, check_url};
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
pub use task_ref::{TaskId, TaskLabel, TaskRef};
pub use traits::{Editable, MarkdownBody};
pub(crate) use traits::{Named, ProjectGroup, Table, read_wrapped, require_name, require_no_comma};

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
