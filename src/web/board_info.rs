//! The info tab of a board (`/board/{id}/info`): its description and its
//! projects. Nothing here has rules of its own: it is all shared components
//! (`ui/`).

use super::Id;
use super::board_header::{BoardSection, board_header};
use super::load::board_of;
use super::open_db;
use super::project_rows::projects_section;
use super::ui::columns::info_body;
use super::ui::frame::frame;
use super::ui::group_card::NO_PROJECTS;
use super::ui::prose::description;
use super::url::BOARDS;
use crate::models::{Board, Project};
use topcoat::{
    Result,
    context::Cx,
    router::{RouterBuilder, page, path_param},
    view::{View, component, view},
};

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder.page(info_page)
}

#[page("/board/{id}/info")]
async fn info_page(cx: &Cx) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let (board, projects) = {
        let db = open_db()?;
        (board_of(&db, id)?, db.projects_in::<Board>(id)?)
    };
    Ok(view! { screen(board: &board, projects: &projects) })
}

#[component]
async fn screen(board: &Board, projects: &[Project]) -> Result<impl View> {
    let title = [BoardSection::Info.label(), board.name.as_str()];
    Ok(view! {
        frame(
            title: &title,
            current: BOARDS,
            board_header(board: board, section: BoardSection::Info)
            info_body(
                description(text: &board.description)
                projects_section(projects: projects, empty: NO_PROJECTS)
            )
        )
    })
}
