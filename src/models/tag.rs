use iter_macros::{MarkdownBody, Table};
use serde::{Deserialize, Serialize};

/// The tag that colours a task's urgent flag, and the colour it starts as.
pub const URGENT_TAG: &str = "Urgent";
/// The tag that colours a task's important flag, and the colour it starts as.
pub const IMPORTANT_TAG: &str = "Important";

/// A coloured label. Tags belong to no project or organization: one set is
/// shared by every task in the database.
///
/// The two seeded tags, [`URGENT_TAG`] and [`IMPORTANT_TAG`], are also what
/// the board views colour the Eisenhower flags with.
#[derive(Debug, Clone, Serialize, Deserialize, Table, MarkdownBody)]
#[table(name = "tags", order_by = "name")]
pub struct Tag {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub id: Option<i64>,

    pub name: String,

    /// `#rrggbb`.
    pub color: String,

    /// Free-form markdown notes, edited as the markdown body.
    #[serde(skip)]
    pub description: String,
}

impl Tag {
    /// A blank template for `iter tag new` to open in the editor.
    pub fn template() -> Self {
        Tag {
            id: None,
            name: String::new(),
            color: "#888888".to_string(),
            description: String::new(),
        }
    }

    /// The tags every database is seeded with.
    pub fn defaults() -> [Tag; 2] {
        let seeded = |name: &str, color: &str, description: &str| Tag {
            id: None,
            name: name.to_string(),
            color: color.to_string(),
            description: description.to_string(),
        };
        [
            seeded(URGENT_TAG, "#facc15", "Something is waiting on this."),
            seeded(IMPORTANT_TAG, "#3b82f6", "This moves a goal forward."),
        ]
    }

    /// Whether this is one of the seeded tags the board views rely on.
    pub fn is_builtin(&self) -> bool {
        self.name == URGENT_TAG || self.name == IMPORTANT_TAG
    }
}

/// `#` followed by exactly six hex digits.
pub fn is_hex_color(s: &str) -> bool {
    s.len() == 7
        && s.starts_with('#')
        && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

impl crate::models::Named for Tag {
    fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_colors_are_hash_and_six_digits() {
        assert!(is_hex_color("#facc15"));
        assert!(is_hex_color("#ABCDEF"));
        assert!(!is_hex_color("facc15"));
        assert!(!is_hex_color("#fff"));
        assert!(!is_hex_color("#gggggg"));
        assert!(!is_hex_color("#facc15 "));
    }
}
