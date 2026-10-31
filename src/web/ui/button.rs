//! Buttons: `.button` (one look, `button.css`), with the "+" add row link as
//! the shared instance. A submit button is `<button class="button primary">`
//! (see `form::form_shell`).

use super::{PLUS, icon};
use topcoat::view::Child;
use topcoat::{
    Result,
    view::{View, component, view},
};

/// The link that adds an entity (a "+" then `label`): the first row of a
/// list or a table, which opens the New pop-up (`modal.rs`). Its look is the
/// add row's (`table.css`).
#[component]
pub async fn add_link(href: &str, label: &str) -> Result<impl View> {
    Ok(view! {
        <a class="add-link" href=(href)>(icon(PLUS)) (label)</a>
    })
}

/// The add row's other form: a button that posts to `action` (it adds
/// today's note, or opens it), looking like [`add_link`].
#[component]
pub async fn add_post(action: &str, label: &str) -> Result<impl View> {
    Ok(view! {
        <form class="add-form" method="post" action=(action)>
            <button type="submit" class="add-link">(icon(PLUS)) (label)</button>
        </form>
    })
}

/// A button that posts `fields` (hidden inputs) to `action`: a state change
/// that needs no script (the pick mode's "Schedule here"). `small` is the
/// compact size for rows; `class` is a variant of the form (`slot`, see
/// `pick_here`).
#[component]
pub async fn post_button(
    #[into] action: String,
    fields: Vec<(&'static str, String)>,
    #[default] small: bool,
    #[default] class: &str,
    child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <form class=(format!("post-form {class}")) method="post" action=(action.as_str())>
            for (name, value) in fields.iter() {
                <input type="hidden" name=(*name) value=(value.as_str())>
            }
            <button class=(if small { "button small" } else { "button" }) type="submit">(child)</button>
        </form>
    })
}
