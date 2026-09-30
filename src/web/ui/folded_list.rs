//! A titled list that starts folded (a `<details>`): what is worth knowing
//! is there, but it does not take the screen until asked. Not a drop zone.

use super::count::count_badge;
use topcoat::{
    Result,
    view::{Child, View, component, view},
};

/// `id` is the title's id (unique on the page); `open` unfolds it (when what
/// the page is about is inside).
#[component]
pub async fn folded_list(
    id: &str,
    title: &str,
    count: usize,
    #[default] open: bool,
    child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <details class="folded" if open { open="" }>
            <summary><span id=(id) class="label">(title) count_badge(n: count, noun: "tasks")</span></summary>
            <div class="folded-body">(child)</div>
        </details>
    })
}
