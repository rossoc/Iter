//! Colored chips; the text color comes from `ink.rs`.

use super::ink::chip_style;
use topcoat::{
    Result,
    view::{View, component, view},
};

/// One chip, ready to show (mapping in `web/task_rows.rs`); only a `#rrggbb` `color` is used.
pub struct Chip<'a> {
    pub name: &'a str,
    pub color: &'a str,
}

#[component]
pub async fn tag_chips(chips: &[Chip<'_>]) -> Result<impl View> {
    Ok(view! {
        <ul class="chips" role="list">
            for c in chips.iter() {
                <li>
                    if let Some(style) = chip_style(c.color) {
                        <span class="tag" style=(style)>(c.name)</span>
                    } else {
                        <span class="tag">(c.name)</span>
                    }
                </li>
            }
        </ul>
    })
}
