//! The banner of the pick mode, the way to move a task without dragging:
//! which task, what can be done with it, and a way out. `#pick` is where the
//! card's link lands, so the keyboard starts here. The mode's words are the
//! caller's (Scheduling, Moving, Placing), the matrix says its own (Placing).

use topcoat::{
    Result,
    view::{Child, View, component, view},
};

/// The id of the banner: where a card's pick link lands (`#pick`), so the
/// keyboard starts here.
pub const PICK_ANCHOR: &str = "pick";

/// The id of the banner's message, which names the banner (`aria-labelledby`).
const MESSAGE: &str = "pick-message";

/// `verb` is what is being done ("Moving"; it also picks the banner's accent);
/// `title` the task; `state` where the task is now ("Wed 30 Sep, 09:00"),
/// empty when it is nowhere; `prompt` what to do next ("Choose an hour.");
/// `cancel` leaves the mode; the child nodes are the extra actions (Unschedule).
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
