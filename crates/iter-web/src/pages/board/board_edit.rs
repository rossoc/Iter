//! The board edit form (`/board/{id}/edit`); the page is `edit_page`.

use crate::{
    forms::{
        BoardForm,
        edit::{Loaded, load_among, submit},
    },
    parts::all::*,
    prelude::*,
};
use iter_core::{db::Table, models::Board};

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
pub(super) async fn board_edit(cx: &Cx) -> Result<impl View> {
    let id = path_id(cx)?;
    let page: Loaded<Board, _> = load_among(id, |db, others: &[Board]| {
        Ok(project_options(db, others, id, None)?)
    })?;
    let (board, projects) = (page.row, page.extra);
    Ok(view! {
        screen(board: &board, stored: &board, projects: &projects)
    })
}

#[page(POST "/board/{id}/edit")]
pub(super) async fn board_save(
    cx: &Cx,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Result<impl View> {
    let id = path_id(cx)?;
    let form = BoardForm::parse(&pairs);
    let refused = submit(id, &form, |db, _| {
        Ok(group_options::<Board>(db, id, Some(&form.projects))?)
    })?;
    Ok(view! {
        screen(board: &refused.row, stored: &refused.stored, projects: &refused.extra, error: Some(&refused.error))
    })
}
