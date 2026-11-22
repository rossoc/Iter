//! A POST form: error box, "* Required" note, the controls, then submit and the way out.

use super::form_error::FormError;
use super::notice::error_box;
use topcoat::{
    Result,
    view::{Child, View, component, view},
};

#[component]
async fn required_note() -> Result<impl View> {
    Ok(view! {
        <p class="required-note"><span aria-hidden="true">"*"</span> " Required"</p>
    })
}

#[component]
async fn form_actions(cancel: &str, submit_label: &str) -> Result<impl View> {
    Ok(view! {
        <div class="actions">
            <button type="submit" class="button primary">(submit_label)</button>
            <a class="u" href=(cancel)>"Cancel"</a>
        </div>
    })
}

/// `cancel` is where Cancel goes; `error` the last refusal; `lead` comes before the
/// error box (a hidden default submit button, see `project_new::project_modal`).
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
