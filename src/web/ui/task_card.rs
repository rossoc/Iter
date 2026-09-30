//! One task on a board, as a card: the title (a link to the task), when it
//! is (or how long), its priority flags, its status when work is under way,
//! and one small link to act on it. The card is what `board.js` drags: a
//! draggable card carries `data-task` (the script makes it draggable and
//! shows the grab cursor; without script it is a plain card). What a card says is the caller's business (see
//! `web/board_cards.rs`); this only lays it out.

use super::status::status;
use super::tag_chips::{Chip, tag_chips};
use crate::models::{Priority, TaskStatus};
use topcoat::{
    Result,
    view::{View, component, view},
};

/// The colored left edge: what the card's priority looks like at a glance.
/// Never the only cue (the flags are also chips with their names).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Edge {
    None,
    Urgent,
    Important,
    /// Urgent and important: the two colors, one above the other.
    Both,
}

impl Edge {
    /// The edge of a priority: the one place the two flags become an edge
    /// (a card's, a quadrant's).
    pub fn of(priority: Priority) -> Edge {
        match (priority.urgent, priority.important) {
            (true, true) => Edge::Both,
            (true, false) => Edge::Urgent,
            (false, true) => Edge::Important,
            (false, false) => Edge::None,
        }
    }

    /// The classes that set the edge's two colors (`edge.css`: `--q1`,
    /// `--q2`): what a card and a quadrant's top edge share.
    pub fn modifier(self) -> &'static str {
        match self {
            Edge::None => "edge",
            Edge::Urgent => "edge edge-urgent",
            Edge::Important => "edge edge-important",
            Edge::Both => "edge edge-both",
        }
    }
}

/// The card's own link ("Schedule"). `label` is followed by the card's title
/// as hidden text, so the link's purpose is clear on its own.
pub struct CardAction<'a> {
    pub label: &'a str,
    pub href: String,
}

/// Everything a card shows.
pub struct CardView<'a> {
    pub id: i64,
    pub href: String,
    pub title: &'a str,
    /// `09:00-10:00`, a duration, or empty.
    pub time: String,
    pub edge: Edge,
    pub flags: Vec<Chip<'a>>,
    /// Only shown for work in progress (the others are the default).
    pub status: Option<TaskStatus>,
    pub action: Option<CardAction<'a>>,
    /// The card being moved (the pick mode): `aria-current`, and marked in
    /// words (not only by its look).
    pub picked: bool,
}

/// The id of the card of task `id`: what a link to it (`#card-7`) lands on,
/// so the keyboard is back on the card after an action.
pub fn card_id(id: i64) -> String {
    format!("card-{id}")
}

/// `draggable` gives the card the `data-task` the script drags by; a list
/// that does not take part in a drag (or a page without `board.js`) leaves
/// it off.
#[component]
pub async fn task_card(card: CardView<'_>, #[default] draggable: bool) -> Result<impl View> {
    let id = card_id(card.id);
    let title_id = format!("{id}-title");
    Ok(view! {
        <article id=(id.as_str()) class=(format!("tcard {}", card.edge.modifier())) tabindex="-1" aria-labelledby=(title_id.as_str())
            if draggable { data-task=(card.id.to_string()) }
            if card.picked { aria-current="true" }>
            <a id=(title_id.as_str()) class="tcard-title" href=(card.href.as_str())>(card.title)</a>
            <div class="tcard-meta">
                if card.picked {
                    <span class="tcard-moving">"Being moved"</span>
                }
                if !card.time.is_empty() {
                    <span class="mono">(card.time.as_str())</span>
                }
                if !card.flags.is_empty() {
                    tag_chips(chips: &card.flags)
                }
                if let Some(state) = card.status {
                    status(state: state)
                }
                if let Some(action) = &card.action {
                    <a class="tcard-act" href=(action.href.as_str())>
                        (action.label)
                        <span class="sr">" " (card.title)</span>
                    </a>
                }
            </div>
        </article>
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_priority_has_its_edge() {
        let of = |urgent, important| Edge::of(Priority { urgent, important });
        assert_eq!(of(true, true), Edge::Both);
        assert_eq!(of(false, true), Edge::Important);
        assert_eq!(of(true, false), Edge::Urgent);
        assert_eq!(of(false, false), Edge::None);
        assert_eq!(Edge::Both.modifier(), "edge edge-both");
        assert_eq!(Edge::None.modifier(), "edge");
    }

    #[test]
    fn a_card_is_reached_by_its_task_id() {
        assert_eq!(card_id(7), "card-7");
    }
}
