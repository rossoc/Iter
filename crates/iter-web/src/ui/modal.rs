//! A pop-up without script: the page links to its own URL with a flag
//! (`?new=task`) and the server renders the modal over it. The page behind is
//! `inert`; closing is a link back to the URL without the flag (the ✕, Cancel, the
//! backdrop). `/modal.js` adds Esc and the focus trap; without it Esc does
//! nothing and Tab can leave the pop-up.

use super::{all::*, kit::*};

/// `id` is the heading's unique id; `close` the URL without the modal; children are the body (a form).
#[component]
pub async fn modal(
    id: &str,
    title: &str,
    #[into] close: String,
    child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <div class="modal">
            <a class="modal-backdrop" href=(close.as_str()) tabindex="-1" aria-hidden="true"></a>
            <section class="modal-box" role="dialog" aria-modal="true" aria-labelledby=(id)>
                <header class="modal-head">
                    <h2 id=(id)>(title)</h2>
                    <a class="modal-x" href=(close.as_str())>(icon(X))<span class="sr">"Close"</span></a>
                </header>
                (child)
            </section>
        </div>
    })
}

/// A pop-up holding one form: `modal` over `form_shell`, cancelling to `close`.
#[component]
pub async fn form_modal(
    id: &str,
    title: &str,
    #[into] close: String,
    #[into] action: String,
    submit: &str,
    #[default] error: Option<&FormError>,
    child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        modal(id: id, title: title, close: close.as_str(),
            form_shell(action: action, error: error, cancel: close.as_str(), submit_label: submit, (child))
        )
    })
}
