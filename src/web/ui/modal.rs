//! A pop-up over the page, without script: the page links to its own URL
//! with a flag (`?new=task`), and the server renders the page with the modal
//! on top. The page behind is `inert` (`frame`'s `dialog`), so focus, clicks
//! and screen readers stay in the modal; closing is a link back to the URL
//! without the flag -- the ✕, the form's Cancel, and the blurred backdrop
//! itself. (No script means no Esc.)

use super::{X, icon};
use topcoat::{
    Result,
    view::{Child, View, component, view},
};

/// `id` is the heading's id, unique on the page (`aria-labelledby`); `close`
/// the URL of the page without the modal. The child nodes are the body (a
/// form, with its own Cancel to `close`).
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
