use iter_macros::{MarkdownBody, Table};
use serde::{Deserialize, Serialize};

/// A calendar that holds projects. Boards come in whatever granularity the
/// user wants -- one per project, per organization, per person -- because
/// a board is nothing but the set of projects bound to it: any project in
/// the database can be put on any board.
///
/// `id` is `None` for a not-yet-created board (the blank template opened
/// in the editor); it's filled in once inserted.
#[derive(Debug, Clone, Serialize, Deserialize, Table, MarkdownBody)]
#[table(name = "boards", order_by = "name")]
pub struct Board {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub id: Option<i64>,

    pub name: String,

    /// Free-form markdown notes, edited as the markdown body -- see
    /// `MarkdownBody`.
    #[serde(skip)]
    pub description: String,
}

impl Board {
    /// A blank template for `iter board new` to open in the editor.
    pub fn template() -> Self {
        Board {
            id: None,
            name: String::new(),
            description: String::new(),
        }
    }
}

impl crate::models::Named for Board {
    fn name(&self) -> &str {
        &self.name
    }
}
