//! The pick-mode banner (moving a task without dragging); `#pick` is where the
//! card's link lands so the keyboard starts here.

use topcoat::{
    Result,
    view::{Child, View, component, view},
};

/// Where a card's pick link lands (`#pick`).
pub const PICK_ANCHOR: &str = "pick";

const MESSAGE: &str = "pick-message";

/// `verb` is the action ("Moving"; also picks the accent), `state` where the
/// task is now (empty if nowhere), `prompt` what to do next, `cancel` leaves the
/// mode; children are extra actions.
#[component]
pub async fn pick_bar(
    verb: &str,
    title: &str,
    #[default] state: &str,
    prompt: &str,
    #[into] cancel: String,
    child: Child<'_>,
) -> Result<impl View> {
    let mode = verb.to_lowercase();
    Ok(view! {
        <div id=(PICK_ANCHOR) class="notice pickbar" data-mode=(mode.as_str()) tabindex="-1" role="group" aria-labelledby=(MESSAGE)>
            <p id=(MESSAGE)>
                (verb) " " <strong>(title)</strong>
                if !state.is_empty() {
                    " (now " (state) ")"
                }
                ". " (prompt)
            </p>
            <div class="pick-actions">
                (child)
                <a class="button small" href=(cancel.as_str())>"Cancel"</a>
            </div>
        </div>
    })
}
