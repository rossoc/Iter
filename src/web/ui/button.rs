//! Buttons: `.button` (one look, `button.css`), with the pencil Edit link and
//! the "+" add link as the shared instances. A submit button is `<button class="button primary">`
//! (see `form_actions`).

use super::{PENCIL, PLUS, icon};
use topcoat::view::Child;
use topcoat::{
    Result,
    view::{View, component, view},
};

/// The link that opens an entity's edit form.
#[component]
pub async fn edit_button(#[into] href: String) -> Result<impl View> {
    Ok(view! {
        <a class="button" href=(href.as_str())>(icon(PENCIL)) "Edit"</a>
    })
}

/// The link that adds an entity (a "+" then `label`).
#[component]
pub async fn add_button(#[into] href: String, label: &str) -> Result<impl View> {
    Ok(view! {
        <a class="button" href=(href.as_str())>(icon(PLUS)) (label)</a>
    })
}

/// A button that posts `fields` (hidden inputs) to `action`: a state change
/// that needs no script (the pick mode's "Schedule here"). `small` is the
/// compact size for rows.
#[component]
pub async fn post_button(
    #[into] action: String,
    fields: Vec<(&'static str, String)>,
    #[default] small: bool,
    child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <form class="post-form" method="post" action=(action.as_str())>
            for (name, value) in fields.iter() {
                <input type="hidden" name=(*name) value=(value.as_str())>
            }
            <button class=(if small { "button small" } else { "button" }) type="submit">(child)</button>
        </form>
    })
}
