//! The v2 design as single-feature components, one file per feature, each
//! with its rules in a `.css` file next to it (scoped under `.v2`). A page
//! assembles them; see `doc/web_components.md` for the API of each and the
//! pages that use it.
//!
//! `/v2.css` serves `v2.css` (tokens, themes, links, focus) followed by every
//! component sheet, concatenated at compile time: one cached request.

// A component and its module share a name (`frame::frame`), so a component
// is imported from its module; only the plain types and functions are
// re-exported here.
pub mod board_empty;
pub mod board_root;
pub mod breadcrumb;
pub mod button;
pub mod columns;
pub mod count;
pub mod day_nav;
pub mod drop_zone;
pub mod empty_line;
pub mod empty_state;
pub mod field;
pub mod folded_list;
pub mod form;
pub mod form_actions;
pub mod form_error;
pub mod frame;
pub mod group;
pub mod group_card;
pub mod hour_grid;
mod icons;
mod ink;
pub mod jump_link;
pub mod labelled;
pub mod matrix_grid;
pub mod notice;
pub mod page_header;
pub mod panel;
pub mod pick_bar;
pub mod pick_here;
pub mod project_list;
pub mod prose;
pub mod quadrant;
pub mod sessions_table;
pub mod side_lists;
pub mod status;
pub mod sticky_head;
pub mod tabs;
pub mod tag_chips;
pub mod task_card;
pub mod task_table;
pub mod tasks_tab;
pub mod unplaced_list;

pub use breadcrumb::Crumb;
pub use icons::*;

use super::assets::Asset;
use topcoat::{
    Result,
    context::Cx,
    router::{RouterBuilder, response::Response, route},
};

static CSS: Asset = Asset::css(concat!(
    include_str!("../v2.css"),
    include_str!("frame.css"),
    include_str!("page_header.css"),
    include_str!("button.css"),
    include_str!("breadcrumb.css"),
    include_str!("count.css"),
    include_str!("tabs.css"),
    include_str!("status.css"),
    include_str!("tag_chips.css"),
    include_str!("edge.css"),
    include_str!("task_card.css"),
    include_str!("drop_zone.css"),
    include_str!("folded_list.css"),
    include_str!("jump_link.css"),
    include_str!("sticky_head.css"),
    include_str!("day_nav.css"),
    include_str!("hour_grid.css"),
    include_str!("side_lists.css"),
    include_str!("matrix_grid.css"),
    include_str!("quadrant.css"),
    include_str!("pick_bar.css"),
    include_str!("sessions_table.css"),
    include_str!("task_table.css"),
    include_str!("tasks_tab.css"),
    include_str!("panel.css"),
    include_str!("columns.css"),
    include_str!("prose.css"),
    include_str!("labelled.css"),
    include_str!("project_list.css"),
    include_str!("group_card.css"),
    include_str!("empty_line.css"),
    include_str!("empty_state.css"),
    include_str!("notice.css"),
    include_str!("control.css"),
    include_str!("field.css"),
    include_str!("group.css"),
    include_str!("form.css"),
    include_str!("form_actions.css"),
));

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder.route(stylesheet)
}

#[route(GET "/v2.css")]
async fn stylesheet(cx: &Cx) -> Result<Response> {
    CSS.respond(cx)
}
