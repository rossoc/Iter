//! Board pages: the board list, one board (agenda, matrix), its settings and the drops onto it.

use crate::prelude::*;

pub mod agenda;
pub mod agenda_cards;
pub mod agenda_drop;
pub mod board_cards;
pub mod board_edit;
pub mod board_info;
pub mod board_page;
pub mod boards;
pub mod drop;
pub mod matrix;
pub mod pick;

pub fn routes(builder: RouterBuilder) -> RouterBuilder {
    builder
        .page(agenda::show)
        .route(agenda_drop::schedule_task)
        .page(board_edit::board_edit)
        .page(board_edit::board_save)
        .page(board_info::info_page)
        .page(boards::boards_page)
        .page(matrix::matrix_page)
        .route(matrix::place_task)
}
