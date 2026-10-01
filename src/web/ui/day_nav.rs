//! A day's heading with its way around: previous day, Today, next day. The
//! title is a date field's label: with script, it opens the browser's date
//! picker; without, the field is a plain date input and a hidden button
//! submits it. Everything else is plain links.

use super::{CALENDAR, CHEVRON_LEFT, CHEVRON_RIGHT, icon};
use topcoat::{
    Result,
    view::{Child, View, component, view},
};

/// A link to another day: `label` is its accessible name ("Previous day,
/// Monday 28 September").
#[derive(Clone)]
pub struct DayLink {
    pub href: String,
    pub label: String,
}

/// The id of the date field (`board.js` opens its picker when the title is
/// clicked).
pub const DAY_INPUT: &str = "day-input";

/// `id` is the heading's id (the hours are labelled by it); `title` is the day
/// written out; `iso` is `yyyy-mm-dd` (the `<time>`'s value and the date
/// field's); `action` is the page the date is sent to, and `pick` the task
/// being moved, and `folded` the Unscheduled column folded, both kept
/// through the change of day; `today` is the link to today,
/// marked when it is the day shown (the caller may point it at the current
/// hour). The child nodes are extra controls beside the day links.
#[component]
pub async fn day_nav(
    id: &str,
    title: &str,
    iso: &str,
    #[into] action: String,
    #[default] pick: Option<i64>,
    #[default] folded: bool,
    prev: DayLink,
    next: DayLink,
    #[into] today: String,
    is_today: bool,
    #[default] child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <div class="day-head">
            <form class="day-pick" method="get" action=(action.as_str())>
                <h2 id=(id) class="day-title">
                    <label class="day-label" for=(DAY_INPUT)><time datetime=(iso)>(title)</time> (icon(CALENDAR))</label>
                </h2>
                <input class="day-input" type="date" id=(DAY_INPUT) name="date" value=(iso)>
                if let Some(task) = pick {
                    <input type="hidden" name="pick" value=(task.to_string())>
                }
                if folded {
                    <input type="hidden" name="side" value="off">
                }
                <button class="sr" type="submit">"Go to this date"</button>
            </form>
            <div class="day-tools">
                <nav class="day-nav" aria-label="Change day">
                    <a class="button" href=(prev.href.as_str()) aria-label=(prev.label.as_str()) rel="prev">(icon(CHEVRON_LEFT))</a>
                    <a class="button" href=(today.as_str()) if is_today { aria-current="date" }>"Today"</a>
                    <a class="button" href=(next.href.as_str()) aria-label=(next.label.as_str()) rel="next">(icon(CHEVRON_RIGHT))</a>
                </nav>
                (child)
            </div>
        </div>
    })
}
