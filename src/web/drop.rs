//! What both boards' POST routes share: the posted form, and loading and storing the moved task in one transaction.

use super::open_db;
use crate::models::{Priority, Task};
use serde::Deserialize;
use topcoat::{
    Result,
    router::{
        content::Json,
        error::{RouterErrorExt, bad_request, see_other},
    },
};

/// Pick forms also send where to land back (the agenda's `date`): a page without script has no `board.js` to reload.
#[derive(Deserialize)]
pub struct Drop {
    pub task_id: i64,
    pub target: String,
    pub date: Option<String>,
    pub form: Option<String>,
}

pub fn drop_on(
    board_id: i64,
    drop: &Drop,
    apply: impl FnOnce(&mut Task, &str) -> std::result::Result<Option<Priority>, String>,
) -> Result<Json<bool>> {
    let db = open_db()?;
    let (mut task, project) = super::load::task_with_project(&db, drop.task_id)?;
    (project.board_id == Some(board_id))
        .then_some(())
        .ok_or_not_found()?;
    let flags = apply(&mut task, &drop.target).map_err(bad_request)?;
    db.update_placed(drop.task_id, &task, flags)?;
    Ok(Json(true))
}

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
