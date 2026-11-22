//! Organization pages: the overview, its edit form, the report, sharing time to a platform.

use crate::prelude::*;

pub mod org;
pub mod org_edit;
pub mod org_report;
pub mod org_share;
pub mod platform_pick;
pub mod share_dialog;

pub fn routes(builder: RouterBuilder) -> RouterBuilder {
    let builder = org::assets(builder);
    let builder = platform_pick::assets(builder);
    builder
        .page(org::show)
        .page(org_edit::show)
        .page(org_edit::save)
        .page(org_share::share)
        .route(platform_pick::activities_of)
}
