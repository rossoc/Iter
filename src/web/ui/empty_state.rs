//! "Nothing here", as a dashed box with an icon and a hint that may hold
//! `<code>`. The quiet one-liner is `empty_line`.

use super::icon;
use topcoat::{
    Result,
    view::{Child, View, component, view},
};

/// `svg` is one of the icon constants.
#[component]
pub async fn empty_state(svg: &'static str, child: Child<'_>) -> Result<impl View> {
    Ok(view! {
        <div class="empty">
            (icon(svg))
            <p>(child)</p>
        </div>
    })
}
