//! One Eisenhower quadrant: a section with number, name, count and rule, holding
//! a drop zone. Text carries the meaning, so color is never the only cue.

use super::count::count_badge;
use super::drop_zone::drop_zone;
use super::empty_line::empty_line;
use super::task_card::Edge;
use topcoat::{
    Result,
    view::{Child, View, component, view},
};

/// `target` is what `board.js` posts for a drop; `number` is 1-4; `rule` what
/// puts a task here; `empty` the line shown when `count` is 0; children are the cards.
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
