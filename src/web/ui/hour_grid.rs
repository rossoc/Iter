//! The hours of a day: rows of hour label and drop zone; today's current hour carries the now line.

use super::drop_zone::drop_zone;
use topcoat::{
    Result,
    view::{Child, View, component, view},
};

/// The now line: how far into the hour (0 to 1) and its hidden text.
pub struct Now {
    pub at: f32,
    pub text: String,
}

/// `label_id` is the id of the heading that names the day.
#[component]
pub async fn hour_grid(label_id: &str, child: Child<'_>) -> Result<impl View> {
    Ok(view! { <ol class="hours" aria-labelledby=(label_id)>(child)</ol> })
}

/// One hour: `label` "09:00", `target` is what a drop posts; children are the cards.
#[component]
pub async fn hour_slot(
    label: &str,
    #[into] target: String,
    #[default] now: Option<Now>,
    child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <li class="hour" if now.is_some() { id="now" aria-current="time" }>
            <span class="hour-label mono">(label)</span>
            drop_zone(target: target, (child))
            if let Some(now) = &now {
                <span class="now" style=(format!("--at:{:.3}", now.at))><span class="sr">(now.text.as_str())</span></span>
            }
        </li>
    })
}
