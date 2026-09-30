//! The board edit form.

use super::{DESCRIPTION, NAME, PROJECTS, Pairs, text};
use crate::db::Db;
use crate::error::Result;
use crate::models::{Board, Named};
use crate::web::edit::EditForm;
use crate::web::url::board_info_url;

/// What the board edit form submits: the same way as [`OrgForm`].
pub struct BoardForm {
    pub name: String,
    pub description: String,
    /// The ids of the projects ticked.
    pub projects: Vec<i64>,
}

impl BoardForm {
    pub fn parse(pairs: &[(String, String)]) -> BoardForm {
        let pairs = Pairs(pairs);
        BoardForm {
            name: pairs.first(NAME),
            description: pairs.first(DESCRIPTION),
            projects: pairs.ids(PROJECTS),
        }
    }
}

impl EditForm for BoardForm {
    type Row = Board;

    fn saved(id: i64) -> String {
        board_info_url(id)
    }

    fn apply(&self, mut board: Board) -> (Board, Result<()>) {
        board.name = self.name.trim().to_string();
        board.description = text(&self.description);
        let valid = board.validate();
        (board, valid)
    }

    /// The projects on the board too, in the same transaction.
    fn save(&self, db: &Db, id: i64, board: &Board) -> Result<()> {
        db.save_group(id, board, &self.projects)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::IterError;
    use crate::web::forms::NAME;

    fn pairs(v: &[(&str, &str)]) -> Vec<(String, String)> {
        v.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn a_board_form_reads_repeated_project_fields() {
        let form = BoardForm::parse(&pairs(&[
            ("name", " work "),
            ("description", "a\r\nb"),
            ("project", "3"),
            ("project", "x"),
            ("project", "5"),
        ]));
        assert_eq!(form.projects, [3, 5]);
        let dup = BoardForm::parse(&pairs(&[
            ("project", "5"),
            ("project", "3"),
            ("project", "5"),
        ]));
        assert_eq!(dup.projects, [3, 5]);
        let board = Board {
            id: Some(1),
            name: String::new(),
            description: String::new(),
        };
        let (board, valid) = form.apply(board);
        assert!(valid.is_ok());
        assert_eq!(
            (board.name.as_str(), board.description.as_str()),
            ("work", "a\nb")
        );
        let (_, blank) = BoardForm::parse(&pairs(&[("name", " ")])).apply(board);
        assert!(matches!(blank, Err(IterError::EmptyName("board"))));
    }

    /// The board errors that belong to a field say which, and a taken name
    /// reads as a sentence.
    #[test]
    fn board_errors_name_their_field() {
        let taken = IterError::NameTaken { kind: "board" };
        assert_eq!(BoardForm::field(&taken), Some(NAME));
        assert_eq!(
            BoardForm::refusal(&taken).message,
            "A board with this name already exists."
        );
        assert_eq!(BoardForm::field(&IterError::EmptyName("board")), Some(NAME));
        assert_eq!(
            BoardForm::field(&IterError::CommandFailed("x".into())),
            None
        );
    }
}
