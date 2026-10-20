//! A board page (the agenda, the matrix): the same lookups for both, each
//! page only saying how its cards are split, and the same screen around
//! what each draws.

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

/// The screen of a board page: the frame, the board's heading and tabs, and
/// the drag-and-drop area (`post`, `style` and `note` as `board_root` takes
/// them) with the page's own part (the child nodes) beside its `side` lists;
/// `folded` when those are folded into their rail. `empty`: the board has no
/// unfinished task, and the page says so instead.
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
