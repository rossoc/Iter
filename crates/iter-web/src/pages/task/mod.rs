//! Task pages: the overview, edit and create forms, a note's or sketch's own page, and the notes and sessions pop-ups.

use crate::prelude::*;

#[allow(clippy::module_inception)] // the page itself, beside its siblings
pub mod task;
pub mod task_edit;
pub mod task_new;
pub mod task_note;
pub mod task_notes;
pub mod task_panel;
pub mod task_sessions;

pub fn routes(builder: RouterBuilder) -> RouterBuilder {
    builder
        .page(task::show)
        .page(task_edit::show)
        .page(task_edit::save)
        .page(task_new::old_form)
        .page(task_new::save)
        .page(task_note::note)
        .page(task_note::sketch)
        .page(task_note::note_save)
        .route(task_notes::add_note)
        .route(task_sessions::add_session)
}
