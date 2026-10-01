//! The button of the pick mode that puts the picked task in a place ("Schedule
//! here", "Move here"): a form of hidden fields, no script. `at` names the
//! place for a screen reader ("11:00").

use super::button::post_button;
use topcoat::{
    Result,
    view::{View, component, view},
};

/// `verb` is "Schedule" / "Move" / "Place"; `fields` are what the form posts
/// (see `web/pick.rs`). A `slot` fills its place (the agenda's hour) with a
/// big dashed box that names the place in words; any other is a small button.
#[component]
pub async fn pick_here(
    #[into] action: String,
    fields: Vec<(&'static str, String)>,
    verb: &str,
    at: &str,
    #[default] slot: bool,
) -> Result<impl View> {
    Ok(view! {
        if slot {
            post_button(action: action, fields: fields, class: "slot",
                (verb) " here \u{b7} " (at))
        } else {
            post_button(action: action, fields: fields, small: true,
                (verb) " here" <span class="sr">" at " (at)</span>)
        }
    })
}
