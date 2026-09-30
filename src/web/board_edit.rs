//! The board edit form (`/board/{id}/edit`), the sibling of the organization,
//! project and task ones. The page is `edit_page`; this supplies the crumbs,
//! the Projects checklist and the loading.

use super::Id;
use super::crumbs::board_edit_trail;
use super::edit::{Loaded, load_among, submit};
use super::edit_page::edit_page;
use super::forms::BoardForm;
use super::project_options::{group_options, project_checklist, project_options};
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

/// `stored` is the board as stored (the breadcrumb and title of the page, not
/// what was typed); `board` holds what the form shows and `projects` the
/// checklist's options. `error` is the refusal, if any.
#[component]
pub async fn screen(
    board: &Board,
    stored: &Board,
    projects: &[CheckOption],
    #[default] error: Option<&FormError>,
) -> Result<impl View> {
    let base = board_info_url(stored.id());
    let crumbs = board_edit_trail(&stored.name, &base);
    Ok(view! {
        edit_page(
            kind: "board",
            subject: &stored.name,
            name: &board.name,
            description: &board.description,
            crumbs: &crumbs,
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
    // The board is found among the boards: no read by key of its own.
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
