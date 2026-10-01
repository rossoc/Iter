//! The day's split of a board's cards: which are waiting (important or
//! not), which sit at which hour of the shown day, which are overdue. A task
//! scheduled for a later day than the one shown is not shown at all (it is not
//! a worry yet). Pure functions, no markup.

use crate::models::{Card, START_TIME_FMT, TaskStatus};
use chrono::{NaiveDate, Timelike};

/// A board's unfinished cards, sorted for one day.
#[derive(Default)]
pub struct Day {
    /// Not scheduled, flagged important.
    pub important: Vec<Card>,
    /// Not scheduled, urgent but not important.
    pub other: Vec<Card>,
    /// Not scheduled, neither urgent nor important: the backlog.
    pub backlog: Vec<Card>,
    /// Scheduled on the day, by the hour they start in.
    pub hours: [Vec<Card>; 24],
    /// Scheduled before today and still open, oldest first: they are on no
    /// hour of this page (unless it is their day), so they get a list of
    /// their own.
    pub overdue: Vec<Card>,
    /// How many start after today, on a day other than the one shown: not
    /// shown, but they are tasks of the board (the page is not empty).
    pub later: usize,
}

impl Day {
    /// How many tasks the Unscheduled column holds: those waiting to be
    /// scheduled (the three lists) and the overdue ones.
    pub fn unscheduled(&self) -> usize {
        self.important.len() + self.other.len() + self.backlog.len() + self.overdue.len()
    }

    /// No card at all: the board has no unfinished task.
    pub fn is_empty(&self) -> bool {
        self.important.is_empty()
            && self.other.is_empty()
            && self.backlog.is_empty()
            && self.overdue.is_empty()
            && self.later == 0
            && self.hours.iter().all(Vec::is_empty)
    }
}

/// Sorts `cards` (already in list order) in one pass: a card with no start
/// goes to a list, one starting on `day` to its hour, one that started before
/// `today` (and is not on `day`) to `overdue`, any other (a later day) is
/// left out. Within an hour, cards are ordered by start; the sort is stable,
/// so equal starts keep the list order. The three lists (the two of waiting
/// cards and `overdue`) put the urgent cards first, then those in progress
/// (`rank`); `overdue` then goes by start, oldest first.
pub fn split(cards: Vec<Card>, day: NaiveDate, today: NaiveDate) -> Day {
    let mut out = Day::default();
    for card in cards {
        match card.task.start_time {
            None if card.priority.important => out.important.push(card),
            None if card.priority.urgent => out.other.push(card),
            None => out.backlog.push(card),
            Some(start) if start.date() == day => out.hours[start.hour() as usize].push(card),
            Some(start) if start.date() < today => out.overdue.push(card),
            Some(_) => out.later += 1,
        }
    }
    for hour in out.hours.iter_mut().filter(|h| h.len() > 1) {
        hour.sort_by_key(|c| c.task.start_time);
    }
    out.overdue.sort_by_key(|c| (rank(c), c.task.start_time));
    out.important.sort_by_key(rank);
    out.other.sort_by_key(rank);
    out.backlog.sort_by_key(rank);
    out
}

/// Where a waiting card sits in its list: the urgent ones first, then those
/// in progress, then the rest (the sort is stable, so each group keeps the
/// list order).
fn rank(card: &Card) -> (bool, bool) {
    (!card.priority.urgent, card.task.status != TaskStatus::Wip)
}

/// The days before and after `day` (`day` itself at the ends of the calendar).
pub fn neighbours(day: NaiveDate) -> (NaiveDate, NaiveDate) {
    (day.pred_opt().unwrap_or(day), day.succ_opt().unwrap_or(day))
}

/// The label of hour `h`: "09:00".
pub fn hour_label(h: usize) -> String {
    format!("{h:02}:00")
}

/// What a drop on hour `h` of `day` posts: `yyyy-mm-dd hh:mm`.
pub fn hour_target(day: NaiveDate, h: usize) -> String {
    day.and_hms_opt(h as u32, 0, 0)
        .expect("hour is in range")
        .format(START_TIME_FMT)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Priority, Task, parse_start_time};

    fn card(label: &str, start: Option<&str>, important: bool) -> Card {
        let mut task = Task::template(1, String::new(), TaskStatus::Queue);
        task.start_time = start.and_then(parse_start_time);
        Card {
            label: label.into(),
            task,
            priority: Priority {
                urgent: false,
                important,
            },
        }
    }

    fn labels(cards: &[Card]) -> Vec<&str> {
        cards.iter().map(|c| c.label.as_str()).collect()
    }

    fn day() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 9, 29).unwrap()
    }

    #[test]
    fn cards_go_to_their_list_or_their_hour() {
        let out = split(
            vec![
                card("a", None, true),
                card("b", None, false),
                card("c", Some("2026-09-29 09:30"), false),
                card("d", Some("2026-09-29 00:00"), true),
                card("e", Some("2026-09-29 23:59"), false),
                card("f", Some("2026-09-30 09:00"), false),
                card("g", Some("2026-09-28 09:00"), false),
            ],
            day(),
            day(),
        );
        assert_eq!(labels(&out.important), ["a"]);
        assert_eq!(labels(&out.backlog), ["b"]);
        assert!(out.other.is_empty());
        assert_eq!(labels(&out.hours[9]), ["c"]);
        assert_eq!(labels(&out.hours[0]), ["d"]);
        assert_eq!(labels(&out.hours[23]), ["e"]);
        // g is before today: overdue; f is later: left out
        let shown: usize = out.hours.iter().map(Vec::len).sum();
        assert_eq!(shown, 3);
        assert_eq!(labels(&out.overdue), ["g"]);
        assert_eq!(out.later, 1);
        assert!(!out.is_empty());
    }

    #[test]
    fn a_list_puts_urgent_first_then_in_progress() {
        let with = |label: &str, urgent: bool, wip: bool| {
            let mut c = card(label, None, true);
            c.priority.urgent = urgent;
            if wip {
                c.task.status = TaskStatus::Wip;
            }
            c
        };
        let out = split(
            vec![
                with("plain", false, false),
                with("wip", false, true),
                with("urgent-wip", true, true),
                with("urgent", true, false),
                with("also-plain", false, false),
            ],
            day(),
            day(),
        );
        assert_eq!(
            labels(&out.important),
            ["urgent-wip", "urgent", "wip", "plain", "also-plain"]
        );
    }

    #[test]
    fn overdue_puts_urgent_and_in_progress_first_then_the_oldest() {
        let with = |label: &str, start: &str, urgent: bool| {
            let mut c = card(label, Some(start), true);
            c.priority.urgent = urgent;
            c
        };
        let out = split(
            vec![
                with("old", "2026-09-20 09:00", false),
                with("urgent-new", "2026-09-28 09:00", true),
                with("urgent-old", "2026-09-21 09:00", true),
            ],
            day(),
            day(),
        );
        assert_eq!(labels(&out.overdue), ["urgent-old", "urgent-new", "old"]);
    }

    #[test]
    fn an_urgent_card_that_is_not_important_is_not_in_the_backlog() {
        let mut urgent = card("u", None, false);
        urgent.priority.urgent = true;
        let out = split(vec![urgent, card("n", None, false)], day(), day());
        assert_eq!(labels(&out.other), ["u"]);
        assert_eq!(labels(&out.backlog), ["n"]);
        assert_eq!(out.unscheduled(), 2);
    }

    #[test]
    fn a_board_without_cards_is_empty() {
        assert!(split(Vec::new(), day(), day()).is_empty());
        assert!(
            !split(
                vec![card("a", Some("2026-09-01 09:00"), false)],
                day(),
                day()
            )
            .is_empty()
        );
        // a task that is later is hidden but still a task of the board
        assert!(
            !split(
                vec![card("a", Some("2026-12-01 09:00"), false)],
                day(),
                day()
            )
            .is_empty()
        );
    }

    #[test]
    fn past_is_against_today_not_the_day_shown() {
        let cards = || {
            vec![
                card("old", Some("2026-09-20 09:00"), false),
                card("yesterday", Some("2026-09-28 09:00"), false),
                card("today", Some("2026-09-29 09:00"), false),
                card("soon", Some("2026-10-02 09:00"), false),
            ]
        };
        // looking at a past day: today's card is still not overdue, nor shown
        let out = split(
            cards(),
            NaiveDate::from_ymd_opt(2026, 9, 10).unwrap(),
            day(),
        );
        assert_eq!(labels(&out.overdue), ["old", "yesterday"]);
        assert_eq!(out.later, 2);
        // the day shown wins: a future day shows its own hours
        let soon = NaiveDate::from_ymd_opt(2026, 10, 2).unwrap();
        let out = split(cards(), soon, day());
        assert_eq!(labels(&out.hours[9]), ["soon"]);
        assert_eq!(labels(&out.overdue), ["old", "yesterday"]);
        assert_eq!(out.later, 1);
    }

    #[test]
    fn the_neighbours_are_the_days_around() {
        let (before, after) = neighbours(day());
        assert_eq!(before, NaiveDate::from_ymd_opt(2026, 9, 28).unwrap());
        assert_eq!(after, NaiveDate::from_ymd_opt(2026, 9, 30).unwrap());
        assert_eq!(neighbours(NaiveDate::MAX).1, NaiveDate::MAX);
    }

    #[test]
    fn an_hour_is_ordered_by_start_and_keeps_list_order_for_ties() {
        let out = split(
            vec![
                card("late", Some("2026-09-29 09:45"), false),
                card("early", Some("2026-09-29 09:00"), false),
                card("tie-1", Some("2026-09-29 09:15"), false),
                card("tie-2", Some("2026-09-29 09:15"), false),
            ],
            day(),
            day(),
        );
        assert_eq!(labels(&out.hours[9]), ["early", "tie-1", "tie-2", "late"]);
    }

    #[test]
    fn an_hour_is_labelled_and_targeted() {
        assert_eq!(hour_label(9), "09:00");
        assert_eq!(hour_label(23), "23:00");
        assert_eq!(hour_target(day(), 7), "2026-09-29 07:00");
        assert!(parse_start_time(&hour_target(day(), 23)).is_some());
    }
}
