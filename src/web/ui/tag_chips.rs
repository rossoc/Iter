//! Colored chips: a list of names in their own colors. The text color is
//! computed from the color (see `ink.rs`) so the text always reads.

use super::ink::chip_style;
use topcoat::{
    Result,
    view::{View, component, view},
};

/// One chip, ready to show (see `web/task_rows.rs` for the mapping from
/// tags). `color` is whatever the source stores: only a `#rrggbb` is used.
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
