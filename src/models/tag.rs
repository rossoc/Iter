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
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

impl crate::models::Named for Tag {
    const KIND: &'static str = "tag";

    fn name(&self) -> &str {
        &self.name
    }

    /// Trims the name, lowercases the colour, and holds the colour to
    /// `#rrggbb`.
    fn validate(&mut self) -> crate::error::Result<()> {
        self.name = self.name.trim().to_string();
        self.color = self.color.trim().to_lowercase();
        crate::models::require_name(Self::KIND, &self.name)?;
        match is_hex_color(&self.color) {
            true => Ok(()),
            false => Err(crate::error::IterError::InvalidTagColor(self.color.clone())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::IterError;
    use crate::models::{Board, Named};

    fn tag(name: &str, color: &str) -> Tag {
        Tag {
            name: name.to_string(),
            color: color.to_string(),
            ..Tag::template()
        }
    }

    #[test]
    fn a_tag_is_normalised_before_it_is_checked() {
        let mut t = tag("  work ", " #ABCDEF ");
        t.validate().expect("valid once trimmed");
        assert_eq!((t.name.as_str(), t.color.as_str()), ("work", "#abcdef"));
    }

    #[test]
    fn a_tag_needs_a_name_and_a_hex_colour() {
        assert!(matches!(
            tag(" ", "#ffffff").validate(),
            Err(IterError::EmptyName("tag"))
        ));
        assert!(matches!(
            tag("x", "red").validate(),
            Err(IterError::InvalidTagColor(c)) if c == "red"
        ));
    }

    /// A project is a place on disk, so it needs a path as well as a name.
    #[test]
    fn a_project_needs_a_name_and_a_base_path() {
        use crate::config::ProjectDefaults;
        use crate::models::Project;
        let mut p = Project::template(&ProjectDefaults::default());
        assert!(matches!(p.validate(), Err(IterError::EmptyName("project"))));
        p.name = "p".to_string();
        assert!(matches!(p.validate(), Err(IterError::EmptyBasePath)));
        p.base_path = "/tmp/p".to_string();
        assert!(p.validate().is_ok());
    }

    /// Everything else only has the default check: a name.
    #[test]
    fn the_default_validation_is_a_non_blank_name() {
        let mut board = Board::template();
        assert!(matches!(
            board.validate(),
            Err(IterError::EmptyName("board"))
        ));
        board.name = "work".to_string();
        assert!(board.validate().is_ok());
    }

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
