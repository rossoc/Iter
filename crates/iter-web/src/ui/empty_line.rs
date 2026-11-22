//! "Nothing here", as a quiet line ("No tasks.").

use super::kit::*;

#[component]
pub async fn empty_line(child: Child<'_>) -> Result<impl View> {
    Ok(view! { <p class="empty-line">(child)</p> })
}
