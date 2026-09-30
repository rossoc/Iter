//! The list of boards (`/boards`), the entry to the board section: a grid of
//! cards like Home's. Nothing here has rules of its own: it is all shared
//! components (`ui/`).

use super::load::grouped;
use super::notes::first_line;
use super::open_db;
use super::project_rows::project_items;
use super::ui::KANBAN;
use super::ui::empty_state::empty_state;
use super::ui::frame::frame;
use super::ui::group_card::{NO_PROJECTS, card_grid, group_card};
use super::ui::page_header::{Kicker, lede, page_header};
use super::url::{BOARDS, board_url};
use crate::db::Table;
use crate::models::{Board, Project};
use topcoat::{
    Result,
    router::{RouterBuilder, page},
    view::{View, component, view},
};

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder.page(boards_page)
}

#[page("/boards")]
async fn boards_page() -> Result<impl View> {
    let boards = grouped::<Board>(&open_db()?)?.groups;
    Ok(view! { boards_view(boards: &boards) })
}

/// One card of the grid: the board, its notes' first line, its projects.
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
