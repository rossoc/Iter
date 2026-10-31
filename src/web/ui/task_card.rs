//! One task on a board as a card: title link, start, status when under way, and
//! one action link. `board.js` drags it: a card with `data-task` is made
//! draggable by the script, and is a plain card without.

use super::status::status;
use crate::models::{Priority, TaskStatus};
use topcoat::{
    Result,
    view::{View, component, view},
};

/// The colored left edge for priority; never the only cue (a chip says it too).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Edge {
    None,
    Urgent,
    Important,
    Both,
}

impl Edge {
    /// The edge of a priority (a card's, a quadrant's).
    pub fn of(priority: Priority) -> Edge {
        match (priority.urgent, priority.important) {
            (true, true) => Edge::Both,
            (true, false) => Edge::Urgent,
            (false, true) => Edge::Important,
            (false, false) => Edge::None,
        }
    }

    /// The classes setting the edge's colors (`edge.css`), shared by card and quadrant.
    pub fn modifier(self) -> &'static str {
        match self {
            Edge::None => "edge",
            Edge::Urgent => "edge edge-urgent",
            Edge::Important => "edge edge-important",
            Edge::Both => "edge edge-both",
        }
    }
}

impl Edge {
    /// The flags in words, Urgent first, shown as chips so priority is not only a color.
    pub fn words(self) -> &'static [&'static str] {
        match self {
            Edge::None => &[],
            Edge::Urgent => &["Urgent"],
            Edge::Important => &["Important"],
            Edge::Both => &["Urgent", "Important"],
        }
    }
}

/// The card's link ("Schedule"); `label` is followed by the title as hidden text.
pub struct CardAction<'a> {
    pub label: &'a str,
    pub href: String,
}

pub struct CardView<'a> {
    pub id: i64,
    pub href: String,
    pub title: &'a str,
    /// A date for an overdue card, else empty (the hour row gives the time).
    pub time: String,
    pub edge: Edge,
    /// Shown only for work in progress.
    pub status: Option<TaskStatus>,
    pub action: Option<CardAction<'a>>,
    /// The card being moved (pick mode): `aria-current`, and marked in words.
    pub picked: bool,
}

/// The card's id, the `#card-7` anchor that returns the keyboard to it after an action.
pub fn card_id(id: i64) -> String {
    format!("card-{id}")
}

/// `draggable` gives the card the `data-task` the script drags by.
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
                for word in card.edge.words().iter() {
                    <span class="tcard-prio">(*word)</span>
                }
                if card.picked {
                    <span class="tcard-moving">"Being moved"</span>
                }
                if !card.time.is_empty() {
                    <span class="mono">(card.time.as_str())</span>
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
        assert_eq!(Edge::Both.words(), ["Urgent", "Important"]);
        assert_eq!(Edge::Urgent.words(), ["Urgent"]);
        assert!(Edge::None.words().is_empty());
    }

    #[test]
    fn a_card_is_reached_by_its_task_id() {
        assert_eq!(card_id(7), "card-7");
    }
}
