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

    /// What `iter session new` puts in front of this task's slugified name
    /// to get its branch, e.g. `"feat/"`. Pre-filled from the project's
    /// `branch_template` when the task is created, and editable from then
    /// on -- which is the point of it living on the task rather than being
    /// read off the project at session time. Empty means "whatever the
    /// project's template says now", so a task created before this field
    /// existed still branches the project's way.
    #[serde(default)]
    pub branch_prefix: String,
}

impl Task {
    /// A blank (or issue-prefilled) template for `iter task new` to open in
    /// the YAML editor. `branch_prefix` comes from the owning project's
    /// `branch_template`, so the default is the project's rule and the
    /// editor is where it gets overridden.
    pub fn template(project_id: i64, branch_prefix: String) -> Self {
        Task {
            id: None,
            project_id,
            name: String::new(),
            description: String::new(),
            github_issue: None,
            status: TaskStatus::Queue,
            branch_prefix,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// `branch_prefix` has to reach the YAML editor as an ordinary field --
    /// that's the whole point of it living on the task -- unlike
    /// `project_id`/`description`, which are deliberately kept out of it.
    #[test]
    fn a_new_task_carries_its_branch_prefix_into_the_editor() {
        let template = Task::template(3, "hotfix/".to_string());
        let yaml = serde_yaml::to_string(&template).expect("a task serializes");
        assert!(
            yaml.contains("branch_prefix: hotfix/"),
            "branch_prefix missing from the editor template:\n{yaml}"
        );
        assert!(!yaml.contains("project_id"));
    }

    #[test]
    fn an_edited_prefix_parses_back_out() {
        let task: Task = serde_yaml::from_str("name: t\nbranch_prefix: chore/\n")
            .expect("front matter parses back into a task");
        assert_eq!(task.branch_prefix, "chore/");
    }

    /// Front matter written before the field existed still loads, blank --
    /// which `iter session new` reads as "use the project's template".
    #[test]
    fn a_missing_prefix_defaults_to_blank() {
        let task: Task = serde_yaml::from_str("name: t\n").expect("front matter parses");
        assert_eq!(task.branch_prefix, "");
    }
}
