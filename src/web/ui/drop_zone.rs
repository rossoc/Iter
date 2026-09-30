//! A place cards can be dropped: `target` is what `board.js` posts as the
//! drop's target (an hour's start, the unscheduled lists' empty string, a
//! quadrant's name). Whatever is inside stays where it was until the page
//! reloads. `class` is the variant (`drop-list`: a list that holds cards
//! for good, drawn as a box).

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
