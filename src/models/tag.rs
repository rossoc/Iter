use iter_macros::{MarkdownBody, Table};
use serde::{Deserialize, Serialize};

/// The tag that colours a task's urgent flag, and the colour it starts as.
pub const URGENT_TAG: &str = "Urgent";
/// The tag that colours a task's important flag, and the colour it starts as.
pub const IMPORTANT_TAG: &str = "Important";

/// A task's Eisenhower flags: whether it carries [`URGENT_TAG`] / [`IMPORTANT_TAG`]. The tags are
/// the only record, so flags and colours are one thing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Priority {
    pub urgent: bool,
    pub important: bool,
}

/// A coloured label shared by every task; [`URGENT_TAG`] and [`IMPORTANT_TAG`] are seeded.
#[derive(Debug, Clone, Serialize, Deserialize, Table, MarkdownBody)]
#[table(name = "tags", order_by = "name", kind = "tag", check = "check")]
pub struct Tag {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub id: Option<i64>,

    pub name: String,

    /// `#rrggbb`.
    pub color: String,

    #[serde(skip)]
    pub description: String,
}

impl Tag {
    pub fn template() -> Self {
        Tag {
            id: None,
            name: String::new(),
            color: "#888888".to_string(),
            description: String::new(),
        }
    }

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

    /// Beyond a name: trims it, lowercases the colour, and holds the colour
    /// to `#rrggbb`.
    fn check(&mut self) -> crate::error::Result<()> {
        self.name = self.name.trim().to_string();
        crate::models::require_no_comma("tag", &self.name)?;
        self.color = self.color.trim().to_lowercase();
        match is_hex_color(&self.color) {
            true => Ok(()),
            false => Err(crate::error::IterError::InvalidTagColor(self.color.clone())),
        }
    }

    pub fn is_builtin(&self) -> bool {
        self.name == URGENT_TAG || self.name == IMPORTANT_TAG
    }
}

pub fn is_hex_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
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

    #[test]
    fn a_project_needs_a_name_and_a_base_path() {
        use crate::models::Project;
        use crate::models::Settings;
        let mut p = Project::template(&Settings::default());
        assert!(matches!(p.validate(), Err(IterError::EmptyName("project"))));
        p.name = "p".to_string();
        assert!(matches!(p.validate(), Err(IterError::EmptyBasePath)));
        p.base_path = "/tmp/p".to_string();
        assert!(p.validate().is_ok());
    }

    /// Every write path stores an absolute `base_path`.
    #[test]
    fn a_relative_base_path_is_stored_absolute() {
        use crate::models::{Project, Settings};
        let mut p = Project::template(&Settings::default());
        p.name = "p".to_string();
        p.base_path = " work/p ".to_string();
        p.validate().expect("valid");
        assert!(
            std::path::Path::new(&p.base_path).is_absolute(),
            "{}",
            p.base_path
        );
        assert!(p.base_path.ends_with("work/p"), "{}", p.base_path);
    }

    /// Web forms list these comma-separated.
    #[test]
    fn project_and_tag_names_cannot_hold_a_comma() {
        use crate::models::{Project, Settings};
        assert!(matches!(
            tag("a,b", "#ffffff").validate(),
            Err(IterError::CommaInName { kind: "tag", .. })
        ));
        let mut p = Project::template(&Settings::default());
        p.name = "foo, bar".to_string();
        p.base_path = "/tmp/p".to_string();
        assert!(matches!(
            p.validate(),
            Err(IterError::CommaInName {
                kind: "project",
                ..
            })
        ));
    }

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
