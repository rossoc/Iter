//! The list of boards (`/boards`).

use crate::load::grouped;
use crate::parts::all::*;
use crate::prelude::*;
use iter_core::db::Table;
use iter_core::models::{Board, Project};

#[page("/boards")]
pub(super) async fn boards_page() -> Result<impl View> {
    let boards = grouped::<Board>(&fresh_db()?)?.groups;
    Ok(view! { boards_view(boards: &boards) })
}

#[component]
async fn board_card(board: &Board, projects: &[Project]) -> Result<impl View> {
    let items = project_items(projects);
    Ok(view! {
        group_card(
            title: board.name.as_str(),
            href: Some(board_url(board.id())),
            note: first_line(&board.description),
            items: &items,
            none: NO_PROJECTS
        )
    })
}

#[component]
async fn boards_view(boards: &[(Board, Vec<Project>)]) -> Result<impl View> {
    Ok(view! {
        frame(
            title: &["Boards"],
            current: BOARDS,
            page_header(title: "Boards", kicker: Kicker::Eyebrow("Board"))
            lede("Select a board to plan its tasks.")
            if boards.is_empty() {
                empty_state(
                    svg: KANBAN,
                    "No boards yet. Create one with "<code>"iter board new"</code>"."
                )
            } else {
                card_grid(
                    for (board, projects) in boards.iter() {
                        board_card(board: board, projects: projects)
                    }
                )
            }
        )
    })
}
