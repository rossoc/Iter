use iter_macros::Table;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    #[default]
    Queue,
    Wip,
    Done,
}

impl TaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            TaskStatus::Queue => "queue",
            TaskStatus::Wip => "wip",
            TaskStatus::Done => "done",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "queue" => Some(TaskStatus::Queue),
            "wip" => Some(TaskStatus::Wip),
            "done" => Some(TaskStatus::Done),
            _ => None,
        }
    }
}

/// A unit of work under a project. `project_id` is not part of the YAML
/// editor's view -- it's set by the caller from the `--project` flag (on
/// creation) or looked up from the existing row (on edit), never typed by
/// hand -- so it's skipped on both serialize and deserialize.
#[derive(Debug, Clone, Serialize, Deserialize, Table)]
#[table(name = "tasks", order_by = "name")]
pub struct Task {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub id: Option<i64>,

    #[serde(skip)]
    pub project_id: i64,

    pub name: String,

    /// Free-form markdown notes. Edited below the `---` separator in the
    /// YAML editor rather than as a field among the others -- see
    /// `MarkdownBody`.
    #[serde(skip)]
    pub description: String,

    /// Issue number in the project's repo, if this task tracks one.
    #[serde(default)]
    pub github_issue: Option<i64>,

    #[serde(default)]
    pub status: TaskStatus,
}

impl Task {
    /// A blank (or issue-prefilled) template for `iter task new` to open in
    /// the YAML editor.
    pub fn template(project_id: i64) -> Self {
        Task {
            id: None,
            project_id,
            name: String::new(),
            description: String::new(),
            github_issue: None,
            status: TaskStatus::Queue,
        }
    }
}

impl crate::models::MarkdownBody for Task {
    fn description(&self) -> &str {
        &self.description
    }

    fn set_description(&mut self, description: String) {
        self.description = description;
    }
}
