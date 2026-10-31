//! A titled drop zone of tasks not yet placed; a card dropped in is taken off the
//! day or out of the quadrants (`target` is what `board.js` posts).

use super::count::count_badge;
use super::drop_zone::drop_zone;
use super::empty_line::empty_line;
use topcoat::{
    Result,
    view::{Child, View, component, view},
};

/// `target` is empty on the agenda (unschedule). It is a labelled region unless
/// `alone` in an aside labelled itself (one landmark, not two). `collapsible`
/// makes the heading a `<details>` toggle (open, no script).
#[component]
pub async fn unplaced_list(
    id: &str,
    title: &str,
    empty: &str,
    count: usize,
    #[default] target: &str,
    #[default] alone: bool,
    #[default] collapsible: bool,
    child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <section class="labelled" if !alone { aria-labelledby=(id) }>
            if collapsible {
                <details class="collapsible" open="">
                    <summary><h2 id=(id) class="label">(title) count_badge(n: count, noun: "tasks")</h2></summary>
                    drop_zone(target: target, class: "drop-list",
                        if count == 0 {
                            empty_line((empty))
                        }
                        (child)
                    )
                </details>
            } else {
                <h2 id=(id) class="label">(title) count_badge(n: count, noun: "tasks")</h2>
                drop_zone(target: target, class: "drop-list",
                    if count == 0 {
                        empty_line((empty))
                    }
                    (child)
                )
            }
        </section>
    })
}
