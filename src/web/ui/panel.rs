//! A titled aside of label / value rows: `panel` is the shell; `flag_row`,
//! `text_row`, `chips_row` and `value_row` (any content) are the rows. What
//! the rows say is the caller's business (see `web/settings_panel.rs`,
//! `web/task_panel.rs`).

use super::tag_chips::{Chip, tag_chips};
use super::{CHECK, MINUS, icon};
use topcoat::{
    Result,
    view::{Child, View, component, view},
};

/// `id` is the heading's id, unique on the page (`aria-labelledby`). The
/// child nodes are the rows.
#[component]
pub async fn panel(id: &str, title: &str, child: Child<'_>) -> Result<impl View> {
    Ok(view! {
        <aside class="panel" aria-labelledby=(id)>
            <h2 id=(id) class="label">(title)</h2>
            <dl>(child)</dl>
        </aside>
    })
}

/// One yes/no row.
#[component]
pub async fn flag_row(label: &str, on: bool) -> Result<impl View> {
    Ok(view! {
        <dt>(label)</dt>
        <dd class=(if on { "flag on" } else { "flag" })>
            (icon(if on { CHECK } else { MINUS }))
            (if on { "on" } else { "off" })
        </dd>
    })
}

/// One row whose value is arbitrary content (the child nodes): a status.
#[component]
pub async fn value_row(label: &str, child: Child<'_>) -> Result<impl View> {
    Ok(view! {
        <dt>(label)</dt>
        <dd>(child)</dd>
    })
}

/// The value of a row with nothing to show: "—", read as "Not set".
#[component]
async fn unset() -> Result<impl View> {
    Ok(view! {
        <dd class="unset"><span aria-hidden="true">"—"</span><span class="sr">"Not set"</span></dd>
    })
}

/// One text row, "Not set" when `value` is empty.
#[component]
pub async fn text_row(label: &str, value: &str) -> Result<impl View> {
    Ok(view! {
        <dt>(label)</dt>
        if value.is_empty() {
            unset()
        } else {
            <dd class="mono">(value)</dd>
        }
    })
}

/// One row of chips, "Not set" when there are none.
#[component]
pub async fn chips_row(label: &str, chips: &[Chip<'_>]) -> Result<impl View> {
    Ok(view! {
        <dt>(label)</dt>
        if chips.is_empty() {
            unset()
        } else {
            <dd>tag_chips(chips: chips)</dd>
        }
    })
}
