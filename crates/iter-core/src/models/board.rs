use iter_macros::{MarkdownBody, Table};
use serde::{Deserialize, Serialize};

/// A calendar holding projects: just the set of projects bound to it.
#[derive(Debug, Clone, Serialize, Deserialize, Table, MarkdownBody)]
#[table(name = "boards", order_by = "name", kind = "board")]
pub struct Board {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub id: Option<i64>,

    pub name: String,

    #[serde(skip)]
    pub description: String,
}

impl Board {
    pub fn template() -> Self {
        Board {
            id: None,
            name: String::new(),
            description: String::new(),
        }
    }
}

impl crate::models::ProjectGroup for Board {
    const MEMBER_COLUMN: &'static str = "board_id";
    const NOT_A_MEMBER: &'static str = "isn't bound to a board";
    const DETACHED: &'static str = "on no board";

    fn group_of(project: &crate::models::Project) -> Option<i64> {
        project.board_id
    }
}

/// A task as a board shows it: `<project>/<task>` plus its flags.
#[derive(Debug, Clone)]
pub struct Card {
    pub label: String,
    pub task: crate::models::Task,
    pub priority: crate::models::Priority,
}
