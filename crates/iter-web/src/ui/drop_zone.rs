//! A place cards can be dropped: `target` is what `board.js` posts as the
//! drop's target; `class` is the variant (`drop-list`: drawn as a box).

use topcoat::{
    Result,
    view::{Child, View, component, view},
};

#[component]
pub async fn drop_zone(
    #[into] target: String,
    #[default] class: &str,
    child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <div class=(format!("drop {class}")) data-target=(target.as_str())>(child)</div>
    })
}
