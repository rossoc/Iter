//! The agenda's cards: what a card's link depends on (`Cards`) and the lists
//! of tasks that are not on the day (`side`).

use super::agenda_day::Day;
use super::agenda_pick::links;
use super::board_cards::CardCtx;
use super::ui::board_empty::board_empty;
use super::ui::folded_list::folded_list;
use super::ui::side_lists::side_lists;
use super::ui::side_rail::{side_fold, side_rail};
use super::ui::task_card::{CardView, task_card};
use super::ui::unplaced_list::unplaced_list;
use super::url::with_side;
use crate::db::Table;
use crate::models::Card;
use chrono::NaiveDate;
use topcoat::{
    Result,
    view::{View, component, view},
};

/// The id of the aside of the lists (the jump link's target).
pub const NOT_SCHEDULED: &str = "not-scheduled";

/// The name of the column.
const LABEL: &str = "Unscheduled";

/// What a card's link depends on: the board and the day (a pick link keeps
/// the day), besides what every board card shares.
pub struct Cards {
    pub board: i64,
    pub day: NaiveDate,
    /// The Unscheduled column is folded: a link keeps it so.
    pub folded: bool,
    pub ctx: CardCtx,
}

impl Cards {
    /// The card, with the link to move it (or schedule it).
    pub fn view<'a>(&self, card: &'a Card) -> CardView<'a> {
        let label = if card.task.start_time.is_some() {
            "Move"
        } else {
            "Schedule"
        };
        let href = with_side(
            links(self.board, self.day).pick(card.task.id()),
            self.folded,
        );
        self.ctx.view(card, label, href)
    }

    /// An overdue card: a card shows no time, but the day it was due says
    /// why it is on this list.
    pub fn overdue<'a>(&self, card: &'a Card) -> CardView<'a> {
        let mut view = self.view(card);
        if let Some(start) = card.task.start_time {
            view.time = start.format("%a %-d %b").to_string();
        }
        view
    }
}

/// Where the column folds and unfolds: the page itself, with and without
/// `side=off`.
pub struct Fold {
    pub fold: String,
    pub unfold: String,
}

/// The lists of tasks that are not on the day: those overdue first (scheduled
/// on a day gone by: the most pressing), then those waiting (the drop zones
/// that unschedule), or, when the board has no unfinished task at all, one
/// hint. Folded, only the rail is drawn (`side_rail`): a link to open them,
/// and a drop zone that unschedules.
#[component]
pub async fn side(split: &Day, ctx: &Cards, to: &Fold) -> Result<impl View> {
    Ok(view! {
        if ctx.folded {
            side_rail(href: to.unfold.as_str(), label: LABEL, total: split.unscheduled(), overdue: split.overdue.len())
        } else {
            side_lists(id: NOT_SCHEDULED, label: LABEL,
                side_fold(href: to.fold.as_str(), label: LABEL)
                if split.is_empty() {
                    board_empty()
                } else {
                    if !split.overdue.is_empty() {
                        folded_list(id: "overdue-title", title: "Overdue", count: split.overdue.len(), open: true,
                            for card in split.overdue.iter() {
                                task_card(card: ctx.overdue(card), draggable: true)
                            }
                        )
                    }
                    unplaced_list(id: "important-title", title: "Important, not scheduled", empty: "No important tasks waiting.", count: split.important.len(), collapsible: true,
                        for card in split.important.iter() {
                            task_card(card: ctx.view(card), draggable: true)
                        }
                    )
                    unplaced_list(id: "other-title", title: "Urgent, not scheduled", empty: "No urgent tasks waiting.", count: split.other.len(), collapsible: true,
                        for card in split.other.iter() {
                            task_card(card: ctx.view(card), draggable: true)
                        }
                    )
                    unplaced_list(id: "backlog-title", title: "Backlog", empty: "Nothing in the backlog.", count: split.backlog.len(), collapsible: true,
                        for card in split.backlog.iter() {
                            task_card(card: ctx.view(card), draggable: true)
                        }
                    )
                }
            )
        }
    })
}
