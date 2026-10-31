//! A day's heading with previous, Today and next links. With script the title
//! opens the date picker; without, a hidden button submits the date input.

use super::{CALENDAR, CHEVRON_LEFT, CHEVRON_RIGHT, icon};
use topcoat::{
    Result,
    view::{Child, View, component, view},
};

/// A link to another day; `label` is its accessible name.
#[derive(Clone)]
pub struct DayLink {
    pub href: String,
    pub label: String,
}

/// The id of the date field (`board.js` opens its picker from the title).
pub const DAY_INPUT: &str = "day-input";

/// `pick` (task being moved) and `folded` are kept through a change of day;
/// `today` is marked when shown; child nodes are extra controls.
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
