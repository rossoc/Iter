//! "Nothing here", as a quiet line ("No tasks.").

use topcoat::{
    Result,
    view::{Child, View, component, view},
};

#[component]
pub async fn empty_line(child: Child<'_>) -> Result<impl View> {
    Ok(view! { <p class="empty-line">(child)</p> })
}
