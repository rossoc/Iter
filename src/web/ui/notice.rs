//! A boxed message: a hint or, as `error`, a problem.

use super::field::field_id;
use super::form_error::FormError;
use topcoat::{
    Result,
    view::{Child, Unescaped, View, component, view},
};

/// The id of the form's error notice, which the controls it is about point
/// at (`aria-describedby`).
pub const ERROR_ID: &str = "form-error";

/// `id` names it for `aria-describedby`. `focus` moves focus to it when the
/// page loads (one inline line, which runs as soon as the notice is parsed;
/// `autofocus` was skipped by Chromium on half of the pages a submit came
/// back with), so a screen reader reads it and a keyboard user starts at
/// it; `focus` needs an `id`. An error is always `role="alert"`, so a
/// browser that cannot run the script (no JavaScript) still announces it.
/// The script drops the role just before it moves focus: a notice that takes
/// focus is read as the focus target, and would be read twice with the role.
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
    Ok(view! {
        <p class=(if error { "notice error" } else { "notice" })
            if error { role="alert" }
            if !id.is_empty() { id=(id) }
            if focus { tabindex="-1" }>
            (child)
        </p>
        if focus {
            <script>(Unescaped::new_unchecked(format!("{{const n=document.getElementById('{id}');n.removeAttribute('role');n.focus()}}")))</script>
        }
    })
}

/// The error a form comes back with, if any, focused on load. "Error: " is
/// read first (hidden, since the box's border says it to the eye). When the
/// error is about a control, the message links to it.
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
