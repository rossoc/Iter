//! A boxed message: a hint or, as `error`, a problem.

use super::field::field_id;
use super::form_error::FormError;
use topcoat::{
    Result,
    view::{Child, Unescaped, View, component, view},
};

/// Id of the form's error notice (`aria-describedby` target).
pub const ERROR_ID: &str = "form-error";

/// `focus` moves focus to the notice on load via an inline script (`autofocus`
/// was skipped by Chromium on submit-back pages) and needs an `id`. An error is
/// `role="alert"` so it is announced without script; the script drops the role
/// before focusing, or it would be read twice.
#[component]
pub async fn notice(
    #[default] error: bool,
    #[default] id: &str,
    #[default] focus: bool,
    child: Child<'_>,
) -> Result<impl View> {
    debug_assert!(!focus || !id.is_empty(), "a focused notice needs an id");
    debug_assert!(
        id.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    );
    // The id goes into an inline script unescaped: only characters that cannot end
    // the string pass (`debug_assert!` alone would not hold in release).
    let script_id: String = id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        .collect();
    Ok(view! {
        <p class=(if error { "notice error" } else { "notice" })
            if error { role="alert" }
            if !id.is_empty() { id=(id) }
            if focus { tabindex="-1" }>
            (child)
        </p>
        if focus {
            <script>(Unescaped::new_unchecked(format!("{{const n=document.getElementById('{script_id}');n.removeAttribute('role');n.focus()}}")))</script>
        }
    })
}

/// The form's error, focused on load; "Error: " is read first (hidden), and the message links to its control.
#[component]
pub async fn error_box(error: Option<&FormError>) -> Result<impl View> {
    Ok(view! {
        if let Some(error) = error {
            notice(error: true, id: ERROR_ID, focus: true,
                <span class="sr">"Error: "</span>
                if let Some(name) = error.field {
                    <a class="u" href=(format!("#{}", field_id(name)))>(error.message.as_str())</a>
                } else {
                    (error.message.as_str())
                }
            )
        }
    })
}
