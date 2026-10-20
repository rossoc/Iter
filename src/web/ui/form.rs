//! A POST form and what every form of the app has around its controls: the
//! error box (`notice::error_box`), the "* Required" note, then the controls
//! (`field.rs`, the child nodes), and the submit button with the way out.

use super::form_error::FormError;
use super::notice::error_box;
use topcoat::{
    Result,
    view::{Child, View, component, view},
};

/// "* Required", above the controls of a form that has required ones (the
/// `*` beside their labels, `field.rs`).
#[component]
async fn required_note() -> Result<impl View> {
    Ok(view! {
        <p class="required-note"><span aria-hidden="true">"*"</span> " Required"</p>
    })
}

/// The submit button and the way out of a form.
#[component]
async fn form_actions(cancel: &str, submit_label: &str) -> Result<impl View> {
    Ok(view! {
        <div class="actions">
            <button type="submit" class="button primary">(submit_label)</button>
            <a class="u" href=(cancel)>"Cancel"</a>
        </div>
    })
}

/// `action` is where the form posts and `cancel` where Cancel goes; `error`
/// is the refusal of the last try, if any. `lead` comes first in the form,
/// before the error box (a hidden default submit button, see
/// `project_new::project_modal`). The child nodes are the controls.
#[component]
pub async fn form_shell(
    #[into] action: String,
    error: Option<&FormError>,
    #[into] cancel: String,
    #[default("Save")] submit_label: &str,
    #[default] lead: Option<Child<'_>>,
    child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <form class="form" method="post" action=(action.as_str())>
            if let Some(lead) = lead {
                (lead)
            }
            error_box(error: error)
            required_note()
            (child)
            form_actions(cancel: cancel.as_str(), submit_label: submit_label)
        </form>
    })
}
