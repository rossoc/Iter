//! The info tab of a board (`/board/{id}/info`): its description and its
//! projects. Nothing here has rules of its own: it is all shared components
//! (`ui/`).

use super::Id;
use super::board_header::{BoardSection, board_header};
use super::load::board_of;
use super::open_db;
use super::project_new::{Home, ProjectCreate, project_modal};
use super::project_rows::{project_search, projects_section};
use super::sections::InfoQuery;
use super::ui::columns::info_body;
use super::ui::frame::frame;
use super::ui::group_card::NO_PROJECTS;
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
    let (board, projects) = load(&open_db()?, id)?;
    // `?new=project`: the New project pop-up over the page
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

/// Board `id` and its projects. A 404 for no such board.
pub fn load(db: &Db, id: i64) -> Result<(Board, Vec<Project>)> {
    Ok((board_of(db, id)?, db.projects_in::<Board>(id)?))
}

/// `project_create` is the New project pop-up, when it is open (also when
/// it comes back refused).
#[component]
pub async fn screen(
    board: &Board,
    projects: &[Project],
    #[default] q: Option<&str>,
    #[default] project_create: Option<&ProjectCreate>,
) -> Result<impl View> {
    let add = Home::Board(board.id()).open_url();
    let dialog = project_create.map(|create| Child::new(view! { project_modal(create: create) }));
    let search = project_search(board_info_url(board.id()), q);
    let title = [BoardSection::Info.label(), board.name.as_str()];
    Ok(view! {
        frame(
            title: &title,
            current: BOARDS,
            dialog: dialog,
            board_header(board: board, section: BoardSection::Info)
            info_body(
                description(text: &board.description)
                projects_section(projects: projects, empty: NO_PROJECTS, search: Some(search), add: Some(add))
            )
        )
    })
}
