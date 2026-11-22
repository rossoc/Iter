//! A titled list that folds (a `<details>`); not a drop zone.

use super::count::count_badge;
use topcoat::{
    Result,
    view::{Child, View, component, view},
};

/// `open` unfolds it (when what the page is about is inside).
#[component]
pub async fn folded_list(
    id: &str,
    title: &str,
    count: usize,
    #[default] open: bool,
    child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <section class="labelled" aria-labelledby=(id)>
            <details class="collapsible" if open { open="" }>
                <summary><h2 id=(id) class="label">(title) count_badge(n: count, noun: "tasks")</h2></summary>
                <div class="folded-body">(child)</div>
            </details>
        </section>
    })
}
