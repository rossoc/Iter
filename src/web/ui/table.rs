//! The cells of the app's tables (`task_table`, `sessions_table`). Each
//! repeats its native table semantics as a `role`: on narrow screens the
//! rows are laid out as a grid or as boxes, and browsers then drop a table's
//! semantics.

use topcoat::{
    Result,
    view::{Child, View, component, view},
};

/// A column's header; the child nodes are its name.
#[component]
pub async fn col_head(child: Child<'_>) -> Result<impl View> {
    Ok(view! { <th scope="col" role="columnheader">(child)</th> })
}

/// A cell; `class` is its look (`mono`), `colspan` the columns it spans
/// when more than one.
#[component]
pub async fn cell(
    #[default] class: &str,
    #[default] colspan: &str,
    child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <td if !class.is_empty() { class=(class) } if !colspan.is_empty() { colspan=(colspan) } role="cell">(child)</td>
    })
}
