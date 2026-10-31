//! The board edit form (`/board/{id}/edit`); the page is `edit_page`.

use super::Id;
use super::board_header::{BoardSection, board_tabs};
use super::crumbs::boards_crumb;
use super::edit::{Loaded, load_among, submit};
use super::edit_page::edit_page;
use super::forms::BoardForm;
use super::project_options::{group_options, project_checklist, project_options};
use super::ui::breadcrumb::Crumb;
use super::ui::field::CheckOption;
use super::ui::form_error::FormError;
use super::url::{BOARDS, board_edit_url, board_info_url};
use crate::db::Table;
use crate::models::Board;
use topcoat::{
    Result,
    context::Cx,
    router::{RouterBuilder, content::Form, page, path_param},
    view::{View, component, view},
};

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder.page(board_edit).page(board_save)
}

#[component]
pub async fn screen(
    board: &Board,
    stored: &Board,
    projects: &[CheckOption],
    #[default] error: Option<&FormError>,
) -> Result<impl View> {
    let base = board_info_url(stored.id());
    let crumbs = [boards_crumb(), Crumb::label("Settings")];
    let sections = board_tabs(stored.id(), BoardSection::Settings);
    Ok(view! {
        edit_page(
            kind: "board",
            subject: &stored.name,
            name: &board.name,
            description: &board.description,
            crumbs: &crumbs,
            sections: Some(&sections),
            action: board_edit_url(stored.id()),
            cancel: base,
            error: error,
            current: BOARDS,
            project_checklist(options: projects, noun: "board", error: error)
        )
    })
}

#[page("/board/{id}/edit")]
async fn board_edit(cx: &Cx) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let page: Loaded<Board, _> = load_among(id, |db, others: &[Board]| {
        Ok(project_options(db, others, id, None)?)
    })?;
    let (board, projects) = (page.row, page.extra);
    Ok(view! {
        screen(board: &board, stored: &board, projects: &projects)
    })
}

#[page(POST "/board/{id}/edit")]
async fn board_save(cx: &Cx, Form(pairs): Form<Vec<(String, String)>>) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let form = BoardForm::parse(&pairs);
    let refused = submit(id, &form, |db, _| {
        Ok(group_options::<Board>(db, id, Some(&form.projects))?)
    })?;
    Ok(view! {
        screen(board: &refused.row, stored: &refused.stored, projects: &refused.extra, error: Some(&refused.error))
    })
}
