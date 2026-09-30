//! A day's heading with its way around: previous day, Today, next day. Plain
//! links, so it works without scripts.

use super::{CHEVRON_LEFT, CHEVRON_RIGHT, icon};
use topcoat::{
    Result,
    view::{View, component, view},
};

/// A link to another day: `label` is its accessible name ("Previous day,
/// Monday 28 September").
#[derive(Clone)]
pub struct DayLink {
    pub href: String,
    pub label: String,
}

/// `id` is the heading's id (the hours are labelled by it); `title` is the day written out; `iso` is `yyyy-mm-dd` (the `<time>`'s
/// value); `today` is the link to today, marked when it is the day shown (the caller may point it at the current hour).
#[component]
pub async fn day_nav(
    id: &str,
    title: &str,
    iso: &str,
    prev: DayLink,
    next: DayLink,
    #[into] today: String,
    is_today: bool,
) -> Result<impl View> {
    Ok(view! {
        <div class="day-head">
            <h2 id=(id) class="day-title"><time datetime=(iso)>(title)</time></h2>
            <nav class="day-nav" aria-label="Change day">
                <a class="button" href=(prev.href.as_str()) aria-label=(prev.label.as_str()) rel="prev">(icon(CHEVRON_LEFT))</a>
                <a class="button" href=(today.as_str()) if is_today { aria-current="date" }>"Today"</a>
                <a class="button" href=(next.href.as_str()) aria-label=(next.label.as_str()) rel="next">(icon(CHEVRON_RIGHT))</a>
            </nav>
        </div>
    })
}
