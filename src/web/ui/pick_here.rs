//! The button of the pick mode that puts the picked task in a place ("Schedule
//! here", "Move here"): a form of hidden fields, no script. `at` names the
//! place for a screen reader ("11:00").

use super::button::post_button;
use topcoat::{
    Result,
    view::{View, component, view},
};

/// `verb` is "Schedule" / "Move" / "Place"; `fields` are what the form posts
/// (see `web/pick.rs`).
#[component]
pub async fn pick_here(
    #[into] action: String,
    fields: Vec<(&'static str, String)>,
    verb: &str,
    at: &str,
) -> Result<impl View> {
    Ok(view! {
        post_button(action: action, fields: fields, small: true,
            (verb) " here" <span class="sr">" at " (at)</span>)
    })
}
