//! "Nothing here" as a dashed box with an icon; the one-liner is `empty_line`.

use crate::prelude::*;

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
