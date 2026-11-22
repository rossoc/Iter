//! The agenda's cards and the side lists of tasks that are not on the day.

use crate::{
    pages::board::{
        agenda_pick::links,
        board_cards::{BOARD_EMPTY, CardCtx},
    },
    prelude::*,
};
use chrono::NaiveDate;
use iter_core::{db::Table, front::day::Day, models::Card};

pub const NOT_SCHEDULED: &str = "not-scheduled";

const LABEL: &str = "Unscheduled";

#[derive(Clone, Copy)]
pub struct Cards {
    pub board: i64,
    pub day: NaiveDate,
    pub folded: bool,
    pub ctx: CardCtx,
}

impl Cards {
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

pub struct Fold {
    pub fold: String,
    pub unfold: String,
}

/// The lists of tasks not on the day: overdue first, then waiting (drop zones that unschedule), or one hint when the board has no unfinished task.
#[component]
pub async fn side(split: &Day, ctx: &Cards, to: &Fold) -> Result<impl View> {
    Ok(view! {
        if ctx.folded {
            side_rail(href: to.unfold.as_str(), label: LABEL, total: split.unscheduled(), overdue: split.overdue.len())
        } else {
            side_lists(id: NOT_SCHEDULED, label: LABEL,
                side_fold(href: to.fold.as_str(), label: LABEL)
                if split.is_empty() {
                    empty_state(svg: KANBAN, (BOARD_EMPTY))
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
