//! The agenda's side of the pick mode (`pick.rs` is shared): its words and the day-keeping links.

use super::pick::{PickLinks, PickWords, Picked};
use super::url::board_day_url;
use chrono::NaiveDate;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Priority, parse_start_time};
    use crate::web::url::with_side;

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
