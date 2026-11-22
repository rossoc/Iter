//! Layout atoms: one element and one class each (rules in `layout.css`).

use crate::prelude::*;

/// Two columns (body, then aside), one when narrow; `class` `side-folded` makes the aside a narrow rail.
#[component]
pub async fn info_columns(#[default] class: &str, child: Child<'_>) -> Result<impl View> {
    Ok(view! { <div class=(format!("info {class}"))>(child)</div> })
}

/// What stays under the top bar while a page scrolls; one wrapper so pieces stack without knowing heights.
#[component]
pub async fn sticky_head(child: Child<'_>) -> Result<impl View> {
    Ok(view! { <div class="sticky-head">(child)</div> })
}

/// A link skipping ahead; shown when focused, and always on narrow screens.
#[component]
pub async fn jump_link(#[into] target: String, child: Child<'_>) -> Result<impl View> {
    Ok(view! { <a class="jump" href=(target.as_str())>(child)</a> })
}

/// The aside holding unplaced lists; stays in view so a card can be dragged anywhere.
/// Focusable so a `jump_link` can land on it (`id`); `label` names it.
#[component]
pub async fn side_lists(id: &str, label: &str, child: Child<'_>) -> Result<impl View> {
    Ok(view! { <aside id=(id) class="side" aria-label=(label) tabindex="-1">(child)</aside> })
}
