//! The info tab of a board (`/board/{id}/info`): description and projects.

use super::Id;
use super::board_header::{BoardSection, board_header};
use super::create::{Home, ProjectCreate, project_modal};
use super::fresh_db;
use super::load::board_of;
use super::project_rows::{project_search, projects_section};
use super::sections::InfoQuery;
use super::text::NO_PROJECTS;
use super::ui::frame::frame;
use super::ui::prose::description;
use super::url::{BOARDS, board_info_url};
use crate::db::{Db, Table};
use crate::models::{Board, Project};
use topcoat::{
    Result,
    context::Cx,
    router::{RouterBuilder, page, path_param, query_params},
    view::{Child, View, component, view},
};

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder.page(info_page)
}

#[page("/board/{id}/info")]
async fn info_page(cx: &Cx) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let query = query_params::<InfoQuery>(cx)?;
    let (board, projects) = load(&fresh_db()?, id)?;
    let create = match query.new.as_deref() {
        Some("project") => {
            Some(Home::Board(id).create(query.name.as_deref(), query.base_path.as_deref())?)
        }
        _ => None,
    };
    Ok(
        view! { screen(board: &board, projects: &projects, q: query.q.as_deref(), project_create: create.as_ref()) },
    )
}

pub fn load(db: &Db, id: i64) -> Result<(Board, Vec<Project>)> {
    Ok((board_of(db, id)?, db.projects_in::<Board>(id)?))
}

#[component]
pub async fn screen(
    board: &Board,
    projects: &[Project],
    #[default] q: Option<&str>,
    #[default] project_create: Option<&ProjectCreate>,
) -> Result<impl View> {
    let add = Home::Board(board.id()).open_url();
    let dialog = project_create.map(|create| Child::new(view! { project_modal(create: create) }));
    let refused = project_create.and_then(|c| c.error.as_ref());
    let search = project_search(board_info_url(board.id()), q);
    let title = [BoardSection::Info.label(), board.name.as_str()];
    Ok(view! {
        frame(
            title: &title,
            current: BOARDS,
            error: refused,
            dialog: dialog,
            board_header(board: board, section: BoardSection::Info)
            <div class="info-body">
                description(text: &board.description)
                projects_section(projects: projects, empty: NO_PROJECTS, search: Some(search), add: Some(add))
            </div>
        )
    })
}
