//! A titled block of the Info body: a small-caps heading over its content.

use super::count::count_badge;
use topcoat::{
    Result,
    view::{Child, View, component, view},
};

/// `id` is the heading's id, unique on the page (`aria-labelledby`). The
/// child nodes are the content. `count` adds a badge after the label (as on
/// a tab); `noun` (the plural, "sessions") is what a screen reader hears
/// after the number.
#[component]
pub async fn labelled_section(
    id: &str,
    label: &str,
    #[default] count: Option<usize>,
    #[default] noun: &str,
    child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <section class="labelled" aria-labelledby=(id)>
            <h2 id=(id) class="label">
                (label)
                if let Some(n) = count {
                    count_badge(n: n, noun: noun)
                }
            </h2>
            (child)
        </section>
    })
}

/// A labelled value in mono (a path).
#[component]
pub async fn mono_value(id: &str, label: &str, value: &str) -> Result<impl View> {
    Ok(view! {
        labelled_section(id: id, label: label,
            <p class="value mono"><bdi>(value)</bdi></p>
        )
    })
}
