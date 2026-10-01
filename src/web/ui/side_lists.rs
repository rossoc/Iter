//! The aside beside the day that holds the lists of tasks not yet
//! placed. It stays in view while the day scrolls, so a card can be
//! dragged into any hour. Focusable, so a link (`jump_link`) can land on it.

use topcoat::{
    Result,
    view::{Child, View, component, view},
};

/// `id` is the anchor of the link that skips here; `label` names the aside
/// ("Backlog").
#[component]
pub async fn side_lists(id: &str, label: &str, child: Child<'_>) -> Result<impl View> {
    Ok(view! { <aside id=(id) class="side" aria-label=(label) tabindex="-1">(child)</aside> })
}
