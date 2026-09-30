//! An entity's description: plain text with its line breaks kept.

use topcoat::{
    Result,
    view::{View, component, view},
};

/// Nothing when `text` is empty.
#[component]
pub async fn description(text: &str) -> Result<impl View> {
    Ok(view! {
        if !text.is_empty() {
            <div class="desc">(text)</div>
        }
    })
}
