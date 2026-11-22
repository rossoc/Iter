//! The pick-mode button that puts the picked task in a place: a form of hidden fields, no script.

use super::button::post_button;
use topcoat::{
    Result,
    view::{View, component, view},
};

/// `fields` are what the form posts (see `web/pick.rs`). A `slot` fills its place
/// with a dashed box naming it; any other is a small button.
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
