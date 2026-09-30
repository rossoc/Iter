//! The agenda's side of the pick mode (`pick.rs` is what both boards share):
//! its words (Scheduling / Moving, where the task is now) and the links that
//! keep the day.

use super::pick::{PickLinks, PickWords, Picked};
use super::url::board_day_url;
use chrono::NaiveDate;

const SCHEDULING: PickWords = PickWords {
    verb: "Scheduling",
    action: "Schedule",
};

impl Picked {
    /// It has a start, so it can be unscheduled (and it is moved, not
    /// scheduled).
    pub fn scheduled(&self) -> bool {
        self.start.is_some()
    }

    /// The pick mode's words: a scheduled task is moved, any other scheduled.
    pub fn words(&self) -> PickWords {
        PickWords::of(self.scheduled(), SCHEDULING)
    }

    /// Where the task is, for the banner ("Wed 30 Sep, 09:00") and for a
    /// finished move; empty when it is not scheduled.
    pub fn state(&self) -> String {
        self.start
            .map(|s| s.format("%a %-d %b, %H:%M").to_string())
            .unwrap_or_default()
    }
}

/// The pick-mode links of `day`'s agenda of board `id`.
pub fn links(id: i64, day: NaiveDate) -> PickLinks {
    PickLinks::new(board_day_url(id, day))
}

/// The agenda of `day`, keeping the pick mode (and landing on its banner)
/// when a task is `picked`: what the day links use, so the task can be moved
/// to another day.
pub fn day_url(id: i64, day: NaiveDate, picked: Option<i64>) -> String {
    match picked {
        Some(task) => links(id, day).pick(task),
        None => board_day_url(id, day),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Priority, parse_start_time};

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
    fn leaving_the_mode_and_a_move_land_on_the_card() {
        assert_eq!(links(2, day()).cancel(7), "/board/2?date=2026-09-29#card-7");
        assert_eq!(
            links(2, day()).moved(7),
            "/board/2?date=2026-09-29&moved=7#card-7"
        );
    }
}
