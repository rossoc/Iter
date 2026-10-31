//! The pick mode of the board pages: a card's link (`?pick=<task>`) turns the page into "choose where", the way to move a task without dragging.

use super::ui::pick_bar::PICK_ANCHOR;
use super::ui::task_card::card_id;
use super::url::page_url;
use crate::db::Table;
use crate::models::{Card, Priority};
use chrono::NaiveDateTime;

pub struct Picked {
    pub id: i64,
    pub title: String,
    pub start: Option<NaiveDateTime>,
    pub placed: bool,
    pub priority: Priority,
}

pub fn picked_in(cards: &[Card], wanted: Option<&str>) -> Option<Picked> {
    let wanted: i64 = wanted?.parse().ok()?;
    cards
        .iter()
        .find(|c| c.task.id() == wanted)
        .map(|c| Picked {
            id: wanted,
            title: c.label.clone(),
            start: c.task.start_time,
            placed: c.task.matrix_placed,
            priority: c.priority,
        })
}

pub fn pick_fields(
    task: i64,
    target: &str,
    back: Option<(&'static str, &str)>,
) -> Vec<(&'static str, String)> {
    let mut fields = vec![("task_id", task.to_string()), ("target", target.into())];
    if let Some((name, value)) = back {
        fields.push((name, value.into()));
    }
    fields
}

#[derive(Clone)]
pub struct PickLinks(String);

impl PickLinks {
    pub fn new(base: impl Into<String>) -> PickLinks {
        PickLinks(base.into())
    }

    pub fn pick(&self, task: i64) -> String {
        let mut url = page_url(&self.0, &[("pick", Some(&task.to_string()))]);
        url.push('#');
        url.push_str(PICK_ANCHOR);
        url
    }

    pub fn cancel(&self, task: i64) -> String {
        format!("{}#{}", self.0, card_id(task))
    }

    pub fn moved(&self, task: i64) -> String {
        PickLinks::new(page_url(&self.0, &[("moved", Some(&task.to_string()))])).cancel(task)
    }
}

pub fn moved_note(title: &str, to: &str) -> String {
    format!("Moved {title} to {to}")
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PickWords {
    pub verb: &'static str,
    pub action: &'static str,
}

impl PickWords {
    pub const MOVING: PickWords = PickWords {
        verb: "Moving",
        action: "Move",
    };

    pub fn of(has_place: bool, first: PickWords) -> PickWords {
        if has_place { PickWords::MOVING } else { first }
    }

    pub fn title(self, task: &str) -> String {
        format!("{} {task}", self.verb)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Task, TaskStatus, parse_start_time};

    fn card(id: i64, start: Option<&str>) -> Card {
        let mut task = Task::template(1, String::new(), TaskStatus::Queue);
        task.id = Some(id);
        task.start_time = start.and_then(parse_start_time);
        Card {
            label: format!("p/t{id}"),
            task,
            priority: Priority::default(),
        }
    }

    #[test]
    fn only_a_card_of_the_board_can_be_picked() {
        let cards = [card(1, None), card(2, Some("2026-09-30 09:00"))];
        assert!(picked_in(&cards, None).is_none());
        assert!(picked_in(&cards, Some("x")).is_none());
        assert!(picked_in(&cards, Some("9")).is_none());
        let plain = picked_in(&cards, Some("1")).expect("on the board");
        assert_eq!((plain.title.as_str(), plain.start), ("p/t1", None));
        let scheduled = picked_in(&cards, Some("2")).expect("on the board");
        assert_eq!(scheduled.start, parse_start_time("2026-09-30 09:00"));
    }

    #[test]
    fn the_buttons_post_the_task_the_target_and_where_to_land() {
        assert_eq!(
            pick_fields(7, "2026-09-29 11:00", Some(("date", "2026-09-29"))),
            [
                ("task_id", "7".to_string()),
                ("target", "2026-09-29 11:00".to_string()),
                ("date", "2026-09-29".to_string())
            ]
        );
        assert_eq!(pick_fields(7, "", None).len(), 2);
        assert_eq!(
            pick_fields(7, "both", Some(("form", "1")))[2],
            ("form", "1".to_string())
        );
    }

    #[test]
    fn the_links_keep_the_page_and_land_on_the_banner_or_the_card() {
        let matrix = PickLinks::new("/board/2/matrix");
        assert_eq!(matrix.pick(7), "/board/2/matrix?pick=7#pick");
        assert_eq!(matrix.cancel(7), "/board/2/matrix#card-7");
        assert_eq!(matrix.moved(7), "/board/2/matrix?moved=7#card-7");
        let day = PickLinks::new("/board/2?date=2026-09-29");
        assert_eq!(day.pick(7), "/board/2?date=2026-09-29&pick=7#pick");
        assert_eq!(day.moved(7), "/board/2?date=2026-09-29&moved=7#card-7");
    }

    #[test]
    fn a_move_and_a_pick_are_worded_with_the_task() {
        assert_eq!(moved_note("p/t", "Plan"), "Moved p/t to Plan");
        let placing = PickWords {
            verb: "Placing",
            action: "Place",
        };
        assert_eq!(placing.title("p/t"), "Placing p/t");
        assert_eq!(PickWords::of(false, placing), placing);
        assert_eq!(PickWords::of(true, placing), PickWords::MOVING);
    }
}
