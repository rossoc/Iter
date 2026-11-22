//! The ids and cues that tie a control to its label, hint and the form's error notice.

use crate::prelude::*;

/// Id of the form's error notice (`aria-describedby` target).
pub const ERROR_ID: &str = "form-error";

/// The `id` of the control named `name`.
pub fn field_id(name: &str) -> String {
    format!("f-{name}")
}

/// The `id` of the hint under the control `id`.
pub fn hint_id(id: &str) -> String {
    format!("{id}-hint")
}

/// Cue beside the label for the control the error is about, for those who
/// cannot rely on the border colour. `aria-hidden` and outside the label, so
/// screen readers get `aria-invalid` and the error notice instead.
#[component]
pub async fn invalid_cue() -> Result<impl View> {
    Ok(view! {
        <span class="bad" aria-hidden="true">"Fix this"</span>
    })
}
