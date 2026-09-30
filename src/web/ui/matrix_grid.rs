//! The four quadrants of the Eisenhower matrix, two by two: 1 2 on the first
//! row, 3 4 on the second, in that reading order. When there is no room for
//! two columns (the matrix beside a list on a small screen, or a phone) they
//! stack, still 1 to 4.

use topcoat::{
    Result,
    view::{Child, View, component, view},
};

#[component]
pub async fn matrix_grid(child: Child<'_>) -> Result<impl View> {
    Ok(view! { <div class="quad-grid">(child)</div> })
}
