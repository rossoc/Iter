//! Lookups and screen shared by the agenda and the matrix.

use super::board_cards::{BOARD_EMPTY, Colors};
use super::board_header::{BoardSection, board_header};
use super::load::{board_of, unfinished};
use super::open_db;
use super::pick::{Picked, picked_in};
use super::ui::KANBAN;
use super::ui::board_root::board_root;
use super::ui::empty_state::empty_state;
use super::ui::frame::frame;
use super::ui::layout::info_columns;
use super::url::BOARDS;
use crate::models::{Board, Card};
use topcoat::{
    Result,
    view::{Child, View, component, view},
};

pub struct BoardPage<D> {
    pub board: Board,
    pub colors: Colors,
    pub picked: Option<Picked>,
    pub moved: Option<Picked>,
    pub data: D,
}

/// Unfinished cards of board `id` split by `split`; a `pick`/`moved` task not on the board is ignored.
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

#[component]
pub async fn board_screen(
    board: &Board,
    section: BoardSection,
    title: &[&str],
    post: &str,
    style: &str,
    note: &str,
    folded: bool,
    #[default] empty: bool,
    side: Child<'_>,
    child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        frame(
            title: title,
            current: BOARDS,
            board_header(board: board, section: section)
            if empty {
                empty_state(svg: KANBAN, (BOARD_EMPTY))
            } else {
                board_root(post: post, style: style, note: note,
                    info_columns(class: if folded { "side-folded" } else { "" },
                        <div>(child)</div>
                        (side)
                    )
                )
            }
        )
    })
}
