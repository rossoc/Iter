//! The info tab of a board (`/board/{id}/info`): description and projects.

use crate::load::board_of;
use crate::parts::all::*;
use crate::prelude::*;
use iter_core::db::{Db, Table};
use iter_core::models::{Board, Project};

#[page("/board/{id}/info")]
pub(super) async fn info_page(cx: &Cx) -> Result<impl View> {
    let id = path_id(cx)?;
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
                prose_text(text: &board.description)
                projects_section(projects: projects, empty: NO_PROJECTS, search: Some(search), add: Some(add))
            </div>
        )
    })
}
