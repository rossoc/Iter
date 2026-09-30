//! The two-column layout of an Info tab or the task page: a body on the
//! left, an aside (a `panel`) on the right, one column when narrow.

use topcoat::{
    Result,
    view::{Child, View, component, view},
};

/// A body with no aside (a board's Info): one column of the width the body
/// has beside an aside, so its lines read the same as on the other Info tabs.
#[component]
pub async fn info_body(child: Child<'_>) -> Result<impl View> {
    Ok(view! { <div class="info-body">(child)</div> })
}

/// The child nodes are the body (a `<div>`) followed by the aside.
#[component]
pub async fn info_columns(child: Child<'_>) -> Result<impl View> {
    Ok(view! { <div class="info">(child)</div> })
}
