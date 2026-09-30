//! One quadrant of the Eisenhower matrix: a section named by its number and
//! name (with how many tasks), described by its rule, holding a drop zone
//! for its cards. Its top edge has the colors of the cards' edge; the number,
//! name and rule are text, so color is never the only cue.

use super::count::count_badge;
use super::drop_zone::drop_zone;
use super::empty_line::empty_line;
use super::task_card::Edge;
use topcoat::{
    Result,
    view::{Child, View, component, view},
};

/// `id` is the heading's id (unique on the page); `target` what `board.js`
/// posts for a drop here; `number` its place in the reading order (1-4);
/// `rule` what puts a task here; `empty` the line shown when `count` is 0;
/// the child nodes are the cards (and the pick mode's button).
#[component]
pub async fn quadrant(
    id: &str,
    target: &str,
    number: usize,
    name: &str,
    rule: &str,
    empty: &str,
    edge: Edge,
    count: usize,
    child: Child<'_>,
) -> Result<impl View> {
    let rule_id = format!("{id}-rule");
    Ok(view! {
        <section class=(format!("quad {}", edge.modifier())) aria-labelledby=(id) aria-describedby=(rule_id.as_str())>
            <h2 id=(id) class="quad-head">
                <span class="quad-num mono">(number)</span>
                <span class="quad-name">(name)</span>
                count_badge(n: count, noun: "tasks")
            </h2>
            <p id=(rule_id.as_str()) class="label">(rule)</p>
            drop_zone(target: target, class: "drop-list",
                if count == 0 {
                    empty_line((empty))
                }
                (child)
            )
        </section>
    })
}
