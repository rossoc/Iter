//! The agenda's cards: what a card's link depends on (`Cards`) and the lists
//! of tasks that are not on the day (`side`).

use super::agenda_day::Day;
use super::agenda_pick::links;
use super::board_cards::{CardCtx, Chips};
use super::ui::board_empty::board_empty;
use super::ui::folded_list::folded_list;
use super::ui::side_lists::side_lists;
use super::ui::task_card::{CardView, task_card};
use super::ui::unplaced_list::unplaced_list;
use crate::db::Table;
use crate::models::{Card, IMPORTANT_TAG};
use chrono::NaiveDate;
use topcoat::{
    Result,
    view::{View, component, view},
};

/// The id of the aside of the lists (the jump link's target).
pub const NOT_SCHEDULED: &str = "not-scheduled";

/// What a card's link depends on: the board and the day (a pick link keeps
/// the day), besides what every board card shares.
pub struct Cards<'a> {
    pub board: i64,
    pub day: NaiveDate,
    pub ctx: CardCtx<'a>,
}

impl<'a> Cards<'a> {
    pub fn view(&self, card: &'a Card) -> CardView<'a> {
        self.chips(card, Chips::All)
    }

    fn chips(&self, card: &'a Card, chips: Chips) -> CardView<'a> {
        let label = if card.task.start_time.is_some() {
            "Move"
        } else {
            "Schedule"
        };
        let href = links(self.board, self.day).pick(card.task.id());
        self.ctx.view_chips(card, label, href, chips)
    }

    /// A card of the "Important" list: the list says it is important, so
    /// the chip is left off (Urgent stays).
    pub fn important(&self, card: &'a Card) -> CardView<'a> {
        self.chips(card, Chips::Except(IMPORTANT_TAG))
    }

    /// A card scheduled on another day: its date comes before its time.
    pub fn elsewhere(&self, card: &'a Card) -> CardView<'a> {
        let mut view = self.view(card);
        if let Some(start) = card.task.start_time {
            view.time = format!("{}, {}", start.format("%a %-d %b"), view.time);
        }
        view
    }
}

/// The lists of tasks that are not on the day: those waiting (the drop zones
/// that unschedule), those scheduled another day (folded), or, when the
/// board has no unfinished task at all, one hint.
#[component]
pub async fn side(split: &Day, ctx: &Cards<'_>, open_elsewhere: bool) -> Result<impl View> {
    Ok(view! {
        side_lists(id: NOT_SCHEDULED, label: "Not scheduled",
            if split.is_empty() {
                board_empty()
            } else {
                unplaced_list(id: "important-title", title: "Important, not scheduled", empty: "No important tasks waiting.", count: split.important.len(),
                    for card in split.important.iter() {
                        task_card(card: ctx.important(card), draggable: true)
                    }
                )
                unplaced_list(id: "other-title", title: "Not important, not scheduled", empty: "No other tasks waiting.", count: split.other.len(),
                    for card in split.other.iter() {
                        task_card(card: ctx.view(card), draggable: true)
                    }
                )
                if !split.elsewhere.is_empty() {
                    folded_list(id: "elsewhere-title", title: "Scheduled on other days", count: split.elsewhere.len(), open: open_elsewhere,
                        for card in split.elsewhere.iter() {
                            task_card(card: ctx.elsewhere(card), draggable: true)
                        }
                    )
                }
            }
        )
    })
}
