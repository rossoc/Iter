//! Shared components, one file per feature with its rules in a `.css` next to
//! it; see `doc/web_components.md`. `/style.css` is `base.css`, every component
//! sheet and `org.css`, concatenated at compile time (`web/layout.rs` links it).

// A component shares its module's name (`frame::frame`): import it from the module.
pub mod breadcrumb;
pub mod button;
pub mod count;
pub mod day_nav;
pub mod drop_zone;
pub mod empty_line;
pub mod empty_state;
pub mod field;
pub mod field_ids;
pub mod filter_bar;
pub mod folded_list;
pub mod form;
pub mod form_error;
pub mod frame;
pub mod group;
pub mod group_card;
pub mod hour_grid;
mod icons;
mod ink;
pub mod labelled;
pub mod layout;
pub mod modal;
pub mod notes;
pub mod notice;
pub mod page_header;
pub mod panel;
pub mod pick_bar;
pub mod pick_here;
pub mod project_list;
pub mod prose;
pub mod quadrant;
pub mod search;
pub mod sessions_table;
pub mod side_rail;
pub mod site_footer;
pub mod status;
pub mod table;
pub mod tabs;
pub mod tag_chips;
pub mod task_card;
pub mod task_table;
pub mod tasks_tab;
pub mod toast;
pub mod unplaced_list;

pub use breadcrumb::Crumb;
pub use icons::*;

use super::assets::{Asset, serve};
use topcoat::router::RouterBuilder;

// Order is the cascade: later sheets win, `org.css` last.
static CSS: Asset = Asset::css(concat!(
    include_str!("../base.css"),
    include_str!("frame.css"),
    include_str!("site_footer.css"),
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
    include_str!("layout.css"),
    include_str!("day_nav.css"),
    include_str!("hour_grid.css"),
    include_str!("side_rail.css"),
    include_str!("quadrant.css"),
    include_str!("pick_bar.css"),
    include_str!("pick_here.css"),
    include_str!("table.css"),
    include_str!("sessions_table.css"),
    include_str!("steps.css"),
    include_str!("filter_bar.css"),
    include_str!("panel.css"),
    include_str!("prose.css"),
    include_str!("labelled.css"),
    include_str!("project_list.css"),
    include_str!("group_card.css"),
    include_str!("empty_line.css"),
    include_str!("notes.css"),
    include_str!("empty_state.css"),
    include_str!("notice.css"),
    include_str!("control.css"),
    include_str!("field.css"),
    include_str!("group.css"),
    include_str!("form.css"),
    include_str!("form_actions.css"),
    include_str!("modal.css"),
    include_str!("search.css"),
    include_str!("combobox.css"),
    include_str!("toast.css"),
    include_str!("../org.css"),
));

static NOTICE_JS: Asset = Asset::js(include_str!("notice.js"));
static MODAL_JS: Asset = Asset::js(include_str!("modal.js"));
static FILTER_BAR_JS: Asset = Asset::js(include_str!("filter_bar.js"));
static SEARCH_JS: Asset = Asset::js(include_str!("search.js"));
static COMBOBOX_JS: Asset = Asset::js(include_str!("combobox.js"));
static PATH_PICK_JS: Asset = Asset::js(include_str!("path_pick.js"));
static TOAST_JS: Asset = Asset::js(include_str!("toast.js"));

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    serve(
        builder,
        &[
            ("/style.css", &CSS),
            ("/notice.js", &NOTICE_JS),
            ("/modal.js", &MODAL_JS),
            ("/filter_bar.js", &FILTER_BAR_JS),
            ("/search.js", &SEARCH_JS),
            ("/combobox.js", &COMBOBOX_JS),
            ("/path_pick.js", &PATH_PICK_JS),
            ("/toast.js", &TOAST_JS),
        ],
    )
}
