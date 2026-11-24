//! A titled list that folds (a `<details>`); not a drop zone.

use super::{all::*, kit::*};

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
                <summary><span id=(id) class="label">(title) count_badge(n: count, noun: "tasks")</span></summary>
                <div class="folded-body">(child)</div>
            </details>
        </section>
    })
}
