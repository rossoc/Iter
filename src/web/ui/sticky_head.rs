//! What stays under the top bar while a long page scrolls (the agenda's day
//! heading and, when picking, its banner): one wrapper, so the pieces stack
//! in the page's own order and none needs to know the other's height.
//! Plain when the screen is narrow or short.

use topcoat::{
    Result,
    view::{Child, View, component, view},
};

#[component]
pub async fn sticky_head(child: Child<'_>) -> Result<impl View> {
    Ok(view! { <div class="sticky-head">(child)</div> })
}
