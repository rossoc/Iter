//! Layout atoms: the wrappers that only place what the page puts in them,
//! one element and one class each (rules in `layout.css`).

use topcoat::{
    Result,
    view::{Child, View, component, view},
};

/// The two-column layout of an Info tab, the task page or a board page: the
/// child nodes are the body (a `<div>`) on the left followed by the aside (a
/// `panel`, the side lists) on the right, one column when narrow. `class` is
/// a variant (`side-folded`: the aside is a narrow rail).
#[component]
pub async fn info_columns(#[default] class: &str, child: Child<'_>) -> Result<impl View> {
    Ok(view! { <div class=(format!("info {class}"))>(child)</div> })
}

/// A body with no aside (a board's Info): one column of the width the body
/// has beside an aside, so its lines read the same as on the other Info tabs.
#[component]
pub async fn info_body(child: Child<'_>) -> Result<impl View> {
    Ok(view! { <div class="info-body">(child)</div> })
}

/// What stays under the top bar while a long page scrolls (the agenda's day
/// heading and, when picking, its banner): one wrapper, so the pieces stack
/// in the page's own order and none needs to know the other's height.
/// Plain when the screen is narrow or short.
#[component]
pub async fn sticky_head(child: Child<'_>) -> Result<impl View> {
    Ok(view! { <div class="sticky-head">(child)</div> })
}

/// A link that skips ahead to another part of the page (a list that sits
/// after a long one). Shown when focused, and always when the screen is
/// narrow (the part it skips to is far below there).
#[component]
pub async fn jump_link(#[into] target: String, child: Child<'_>) -> Result<impl View> {
    Ok(view! { <a class="jump" href=(target.as_str())>(child)</a> })
}

/// The aside of a board page that holds the lists of tasks not yet placed.
/// It stays in view while the day or the matrix scrolls, so a card can be
/// dragged anywhere. Focusable, so a `jump_link` can land on it. `id` is the
/// anchor of that link; `label` names the aside ("Backlog").
#[component]
pub async fn side_lists(id: &str, label: &str, child: Child<'_>) -> Result<impl View> {
    Ok(view! { <aside id=(id) class="side" aria-label=(label) tabindex="-1">(child)</aside> })
}

/// The four quadrants of the Eisenhower matrix, two by two: 1 2 on the first
/// row, 3 4 on the second, in that reading order. When there is no room for
/// two columns (the matrix beside a list on a small screen, or a phone) they
/// stack, still 1 to 4.
#[component]
pub async fn matrix_grid(child: Child<'_>) -> Result<impl View> {
    Ok(view! { <div class="quad-grid">(child)</div> })
}
