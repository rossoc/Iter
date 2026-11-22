//! The agenda's side of the pick mode (`pick.rs` is shared): its words, the day-keeping links, its cards and the side lists of tasks that are not on the day.

use crate::{
    pages::board::{
        board_cards::{BOARD_EMPTY, CardCtx},
        pick::{PickLinks, PickWords, Picked},
    },
    prelude::*,
};
use chrono::NaiveDate;
use iter_core::{db::Table, front::day::Day, models::Card};

pub const NOT_SCHEDULED: &str = "not-scheduled";

const LABEL: &str = "Unscheduled";

const SCHEDULING: PickWords = PickWords {
    verb: "Scheduling",
    action: "Schedule",
};

impl Picked {
    pub fn scheduled(&self) -> bool {
        self.start.is_some()
    }

    pub fn words(&self) -> PickWords {
        PickWords::of(self.scheduled(), SCHEDULING)
    }

    pub fn state(&self) -> String {
        self.start
            .map(|s| s.format("%a %-d %b, %H:%M").to_string())
            .unwrap_or_default()
    }
}

pub fn links(id: i64, day: NaiveDate) -> PickLinks {
    PickLinks::new(board_day_url(id, day))
}

pub fn day_url(id: i64, day: NaiveDate, picked: Option<i64>) -> String {
    match picked {
        Some(task) => links(id, day).pick(task),
        None => board_day_url(id, day),
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::url::with_side;
    use iter_core::models::{Priority, parse_start_time};

    fn day() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 9, 29).unwrap()
    }

    fn picked(start: Option<&str>) -> Picked {
        Picked {
            id: 1,
            title: "p/t".into(),
            start: start.and_then(parse_start_time),
            placed: false,
            priority: Priority::default(),
        }
    }

    #[test]
    fn a_scheduled_task_is_moved_and_any_other_scheduled() {
        let plain = picked(None);
        assert_eq!(
            (plain.words().verb, plain.words().action, plain.state()),
            ("Scheduling", "Schedule", String::new())
        );
        let set = picked(Some("2026-09-30 09:00"));
        assert_eq!(
            (set.words().verb, set.words().action, set.state()),
            ("Moving", "Move", "Wed 30 Sep, 09:00".to_string())
        );
    }

    #[test]
    fn the_day_links_keep_the_pick_and_land_on_the_banner() {
        assert_eq!(
            links(2, day()).pick(7),
            "/board/2?date=2026-09-29&pick=7#pick"
        );
        assert_eq!(day_url(2, day(), Some(7)), links(2, day()).pick(7));
        assert_eq!(day_url(2, day(), None), "/board/2?date=2026-09-29");
    }

    #[test]
    fn a_folded_column_is_kept_before_the_fragment() {
        let url = || day_url(2, day(), Some(7));
        assert_eq!(with_side(url(), false), url());
        assert_eq!(
            with_side(url(), true),
            "/board/2?date=2026-09-29&pick=7&side=off#pick"
        );
        assert_eq!(with_side("/board/2".into(), true), "/board/2?side=off");
    }

    #[test]
    fn leaving_the_mode_and_a_move_land_on_the_card() {
        assert_eq!(links(2, day()).cancel(7), "/board/2?date=2026-09-29#card-7");
        assert_eq!(
            links(2, day()).moved(7),
            "/board/2?date=2026-09-29&moved=7#card-7"
        );
    }
}
