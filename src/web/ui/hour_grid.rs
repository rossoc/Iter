//! The hours of a day: an ordered list of rows, each an hour label and a
//! drop zone for the cards at that hour. On today, the row of the current
//! hour carries the now line, drawn from the fraction the caller works out.

use super::drop_zone::drop_zone;
use topcoat::{
    Result,
    view::{Child, View, component, view},
};

/// The now line of an hour: how far into the hour it is (0 to 1) and its
/// hidden text ("Now, 09:25").
pub struct Now {
    pub at: f32,
    pub text: String,
}

/// `label_id` is the id of the heading that names the day.
#[component]
pub async fn hour_grid(label_id: &str, child: Child<'_>) -> Result<impl View> {
    Ok(view! { <ol class="hours" aria-labelledby=(label_id)>(child)</ol> })
}

/// One hour. `label` is "09:00"; `target` is what a drop here posts; the
/// child nodes are the cards (and anything else that belongs in the slot).
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
