//! What both boards' POST routes share (`agenda.rs` `POST /board/{id}/schedule`,
//! `matrix.rs` `POST /board/{id}/matrix`): the form a drop or a pick button
//! posts, loading the task and storing what the page's `apply` makes of it,
//! and the answer.

use super::open_db;
use crate::models::{Priority, Project, Task};
use serde::Deserialize;
use topcoat::{
    Result,
    router::{
        content::Json,
        error::{RouterErrorExt, bad_request, see_other},
    },
};

/// What a drop (or a pick form) posts: the task and where it goes (`target`),
/// as `board.js` posts them, and, only from the pick forms (a page without
/// script has no `board.js` to reload), where to land: the agenda's `date`,
/// the matrix's `form` marker. The names are the ones `pick::pick_fields`
/// writes. One form for both endpoints, not a `#[serde(flatten)]` of a shared
/// part: serde reads a flattened number out of an urlencoded body as text
/// and fails on it.
#[derive(Deserialize)]
pub struct Drop {
    pub task_id: i64,
    pub target: String,
    pub date: Option<String>,
    pub form: Option<String>,
}

/// Loads the dropped task, checks it is one of this board's, and stores what
/// `apply` makes of it -- with the new flags, when it returns some, in one
/// transaction.
pub fn drop_on(
    board_id: i64,
    drop: &Drop,
    apply: impl FnOnce(&mut Task, &str) -> std::result::Result<Option<Priority>, String>,
) -> Result<Json<bool>> {
    let db = open_db()?;
    let mut task = db.get::<Task>(drop.task_id)?.ok_or_not_found()?;
    let project = db.get::<Project>(task.project_id)?.ok_or_not_found()?;
    (project.board_id == Some(board_id))
        .then_some(())
        .ok_or_not_found()?;
    let flags = apply(&mut task, &drop.target).map_err(bad_request)?;
    db.update_placed(drop.task_id, &task, flags)?;
    Ok(Json(true))
}

/// The answer to a move: a redirect to `back` when there is one (a form
/// post), else the JSON `done` (a script's post).
pub fn back_or(done: Json<bool>, back: Option<String>) -> Result<Json<bool>> {
    match back {
        Some(url) => Err(see_other(url).into()),
        None => Ok(done),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_move_answers_json_or_redirects() {
        assert!(back_or(Json(true), None).is_ok());
        assert!(back_or(Json(true), Some("/board/2".into())).is_err());
    }
}
