//! Shared components, one file per feature with its rules in a `.css` next to
//! it; see `doc/web_components.md`. `/style.css` is `base.css`, every component
//! sheet and `org.css`, concatenated at compile time (`pages/site/document.rs` links it).

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
pub mod icons;
pub mod ink;
pub mod kit;
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

pub(crate) mod all {
    pub use super::breadcrumb::*;
    pub use super::button::*;
    pub use super::count::*;
    pub use super::day_nav::*;
    pub use super::drop_zone::*;
    pub use super::empty_line::*;
    pub use super::empty_state::*;
    pub use super::field::*;
    pub use super::field_ids::*;
    pub use super::filter_bar::*;
    pub use super::folded_list::*;
    pub use super::form::*;
    pub use super::form_error::*;
    pub use super::frame::*;
    pub use super::group::*;
    pub use super::group_card::*;
    pub use super::hour_grid::*;
    pub use super::icons::*;
    pub use super::ink::*;
    pub use super::labelled::*;
    pub use super::layout::*;
    pub use super::modal::*;
    pub use super::notes::*;
    pub use super::notice::*;
    pub use super::page_header::*;
    pub use super::panel::*;
    pub use super::pick_bar::*;
    pub use super::pick_here::*;
    pub use super::project_list::*;
    pub use super::prose::*;
    pub use super::quadrant::*;
    pub use super::search::*;
    pub use super::sessions_table::*;
    pub use super::side_rail::*;
    pub use super::site_footer::*;
    pub use super::status::*;
    pub use super::table::*;
    pub use super::tabs::*;
    pub use super::tag_chips::*;
    pub use super::task_card::*;
    pub use super::task_table::*;
    pub use super::tasks_tab::*;
    pub use super::toast::*;
    pub use super::unplaced_list::*;
}

use crate::assets::{Asset, serve};
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
