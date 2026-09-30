//! A list of tasks not yet placed (not on the agenda, not in the matrix):
//! a titled drop zone that holds the cards, or says there are none. A card
//! dropped in it is taken off the day (agenda) or out of the quadrants
//! (matrix); `target` is what `board.js` posts for such a drop.

use super::count::count_badge;
use super::drop_zone::drop_zone;
use super::empty_line::empty_line;
use topcoat::{
    Result,
    view::{Child, View, component, view},
};

/// `id` is the heading's id (unique on the page); `title` the heading;
/// `empty` the line shown when `count` is 0; the child nodes are the cards.
/// `target` is the drop's target (empty on the agenda: unschedule). It is a
/// labelled region, unless it is `alone` in an aside that is labelled itself
/// (the matrix's Not placed): one landmark, not two of the same name.
#[component]
pub async fn unplaced_list(
    id: &str,
    title: &str,
    empty: &str,
    count: usize,
    #[default] target: &str,
    #[default] alone: bool,
    child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <section class="labelled" if !alone { aria-labelledby=(id) }>
            <h2 id=(id) class="label">(title) count_badge(n: count, noun: "tasks")</h2>
            drop_zone(target: target, class: "drop-list",
                if count == 0 {
                    empty_line((empty))
                }
                (child)
            )
        </section>
    })
}
