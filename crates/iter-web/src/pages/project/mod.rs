//! Project pages: the overview, its edit and create forms, the wiki.

use crate::prelude::*;

#[allow(clippy::module_inception)] // the page itself, beside its siblings
pub mod project;
pub mod project_edit;
pub mod project_new;
pub mod project_wiki;

pub fn routes(builder: RouterBuilder) -> RouterBuilder {
    builder
        .page(project::show)
        .page(project_edit::show)
        .page(project_edit::save)
        .page(project_new::quick_org)
        .page(project_new::quick_board)
        .route(project_wiki::act)
}
