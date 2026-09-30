//! Loading a board page (the agenda, the matrix): the same lookups for both;
//! each page only says how its cards are split.

use super::board_cards::Colors;
use super::load::{board_of, unfinished};
use super::open_db;
use super::pick::{Picked, picked_in};
use crate::models::{Board, Card};

/// What a board page draws: the board, the colors, the task
/// being moved (`?pick=`), the one just moved (`?moved=`; the note says so)
/// and the page's own split of the cards.
pub struct BoardPage<D> {
    pub board: Board,
    pub colors: Colors,
    pub picked: Option<Picked>,
    pub moved: Option<Picked>,
    pub data: D,
}

/// Board `id` with its unfinished cards (urgent first, then by label), split
/// by `split` into what the page shows. The tasks `pick` and `moved` name
/// are found among the cards (one that is not on the board is ignored).
pub fn load_board<D>(
    id: i64,
    pick: Option<&str>,
    moved: Option<&str>,
    split: impl FnOnce(Vec<Card>) -> D,
) -> topcoat::Result<BoardPage<D>> {
    let db = open_db()?;
    let board = board_of(&db, id)?;
    let colors = Colors::load(&db)?;
    let cards = unfinished(&db, id)?;
    Ok(BoardPage {
        picked: picked_in(&cards, pick),
        moved: picked_in(&cards, moved),
        board,
        colors,
        data: split(cards),
    })
}
