//! The day's split of a board's cards: which are waiting (important or
//! not), which sit at which hour of the shown day, which sit on another
//! one. Pure functions, no markup.

use crate::models::{Card, START_TIME_FMT};
use chrono::{NaiveDate, Timelike};

/// A board's unfinished cards, sorted for one day.
#[derive(Default)]
pub struct Day {
    /// Not scheduled, flagged important.
    pub important: Vec<Card>,
    /// Not scheduled, not flagged.
    pub other: Vec<Card>,
    /// Scheduled on the day, by the hour they start in.
    pub hours: [Vec<Card>; 24],
    /// Scheduled on another day (earlier or later), soonest first: they are
    /// on no hour of this page, so they get a list of their own.
    pub elsewhere: Vec<Card>,
}

impl Day {
    /// No card at all: the board has no unfinished task.
    pub fn is_empty(&self) -> bool {
        self.important.is_empty()
            && self.other.is_empty()
            && self.elsewhere.is_empty()
            && self.hours.iter().all(Vec::is_empty)
    }
}

/// Sorts `cards` (already in list order) in one pass: a card with no start
/// goes to a list, one starting on `day` to its hour, one starting on another
/// day to `elsewhere`. Within an hour (and in `elsewhere`), cards are ordered
/// by start; the sort is stable, so equal starts keep the list order.
pub fn split(cards: Vec<Card>, day: NaiveDate) -> Day {
    let mut out = Day::default();
    for card in cards {
        match card.task.start_time {
            None if card.priority.important => out.important.push(card),
            None => out.other.push(card),
            Some(start) if start.date() == day => out.hours[start.hour() as usize].push(card),
            Some(_) => out.elsewhere.push(card),
        }
    }
    for hour in out.hours.iter_mut().filter(|h| h.len() > 1) {
        hour.sort_by_key(|c| c.task.start_time);
    }
    out.elsewhere.sort_by_key(|c| c.task.start_time);
    out
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
    use crate::models::{Priority, Task, TaskStatus, parse_start_time};

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
        );
        assert_eq!(labels(&out.important), ["a"]);
        assert_eq!(labels(&out.other), ["b"]);
        assert_eq!(labels(&out.hours[9]), ["c"]);
        assert_eq!(labels(&out.hours[0]), ["d"]);
        assert_eq!(labels(&out.hours[23]), ["e"]);
        // f and g are on other days: in no hour, but in their own list, soonest first
        let shown: usize = out.hours.iter().map(Vec::len).sum();
        assert_eq!(shown, 3);
        assert_eq!(labels(&out.elsewhere), ["g", "f"]);
        assert!(!out.is_empty());
    }

    #[test]
    fn a_board_without_cards_is_empty() {
        assert!(split(Vec::new(), day()).is_empty());
        assert!(!split(vec![card("a", Some("2026-09-01 09:00"), false)], day()).is_empty());
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
