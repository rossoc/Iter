//! Site-wide pages: the document shell, home, settings, search, daily notes, the folder chooser and the live version stream.

use crate::prelude::*;

pub mod daily_notes;
pub mod document;
pub mod folder_picker;
pub mod folders;
pub mod footer;
pub mod home;
pub mod live;
pub mod search_index;
pub mod settings;

pub fn routes(builder: RouterBuilder) -> RouterBuilder {
    document::register(builder)
        .page(daily_notes::project_note)
        .page(daily_notes::project_note_save)
        .route(daily_notes::project_today)
        .page(daily_notes::org_note)
        .page(daily_notes::org_note_save)
        .route(daily_notes::org_today)
        .route(folders::pick_folder)
        .page(home::home)
        .route(live::live)
        .route(search_index::results)
        .page(settings::show)
        .page(settings::save)
}
