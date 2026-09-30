//! A link that skips ahead to another part of the page (a list that sits
//! after a long one). Shown when focused, and always when the screen is
//! narrow (the part it skips to is far below there).

use topcoat::{
    Result,
    view::{Child, View, component, view},
};

#[component]
pub async fn jump_link(#[into] target: String, child: Child<'_>) -> Result<impl View> {
    Ok(view! { <a class="jump" href=(target.as_str())>(child)</a> })
}
