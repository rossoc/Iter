//! The submit button and the way out of a form.

use topcoat::{
    Result,
    view::{View, component, view},
};

#[component]
pub async fn form_actions(
    #[into] cancel: String,
    #[default("Save")] submit_label: &str,
) -> Result<impl View> {
    Ok(view! {
        <div class="actions">
            <button type="submit" class="button primary">(submit_label)</button>
            <a class="u" href=(cancel.as_str())>"Cancel"</a>
        </div>
    })
}
