//! The pick mode of the board pages, the way to move a task without dragging:
//! a card's link (`?pick=<task>`) turns the page into "choose where": a
//! banner names the task and every place gets a button. Plain links and
//! forms, no script. This is what both boards share (which task, the URLs,
//! the fields the buttons post, the words of a finished move); the agenda's
//! words are `agenda_pick.rs`, the matrix's are in `matrix.rs`. The markup is
//! `ui::pick_bar` and `ui::pick_here`.

use super::ui::pick_bar::PICK_ANCHOR;
use super::ui::task_card::card_id;
use super::url::page_url;
use crate::db::Table;
use crate::models::{Card, Priority};
use chrono::NaiveDateTime;

/// A task of the board a page talks about (the one being moved, or the one
/// just moved), with the facts each page words "where it is" with.
pub struct Picked {
    pub id: i64,
    pub title: String,
    /// The agenda: where it is scheduled, if it is.
    pub start: Option<NaiveDateTime>,
    /// The matrix: whether it is in a quadrant, and the flags that say which.
    pub placed: bool,
    pub priority: Priority,
}

/// The card `wanted` (a `pick` or `moved` query) names, if it is one of
/// `cards`.
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

/// What a pick button posts: the task and where it goes (`target`, as
/// `board.js` posts it too), and, when the page has one, a field that says
/// this is a form post and where to land afterwards (`back`: its name and
/// value). The names are the fields of the endpoints' form (`drop.rs`): the
/// task's id, the target, and the agenda's `date` or the matrix's `form`.
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

/// The links of the pick mode of the page at `base` (a board's agenda of a
/// day, or its matrix): one place builds them, for both boards.
pub struct PickLinks(String);

impl PickLinks {
    pub fn new(base: impl Into<String>) -> PickLinks {
        PickLinks(base.into())
    }

    /// Starts the pick mode for `task`, landing on the banner.
    pub fn pick(&self, task: i64) -> String {
        let mut url = page_url(&self.0, &[("pick", Some(&task.to_string()))]);
        url.push('#');
        url.push_str(PICK_ANCHOR);
        url
    }

    /// Leaves the pick mode: the page, on the card of `task`.
    pub fn cancel(&self, task: i64) -> String {
        format!("{}#{}", self.0, card_id(task))
    }

    /// After a move (from a form or a drop): the page, on the card of `task`,
    /// saying it was moved (see [`moved_note`]).
    pub fn moved(&self, task: i64) -> String {
        PickLinks::new(page_url(&self.0, &[("moved", Some(&task.to_string()))])).cancel(task)
    }
}

/// What a move says once done: "Moved Task to Plan". `to` is where the task
/// is now, in the page's words. The status region reads it.
pub fn moved_note(title: &str, to: &str) -> String {
    format!("Moved {title} to {to}")
}

/// The words of the pick mode on a board: `verb` names what is being done
/// in the banner and the document title ("Moving"), `action` is the buttons'
/// ("Move here").
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PickWords {
    pub verb: &'static str,
    pub action: &'static str,
}

impl PickWords {
    /// A task that already has a place is moved, on either board.
    pub const MOVING: PickWords = PickWords {
        verb: "Moving",
        action: "Move",
    };

    /// The words for a task: [`Self::MOVING`] when it `has_place`, else
    /// `first` (the board's word for the first placing: scheduling, placing).
    pub fn of(has_place: bool, first: PickWords) -> PickWords {
        if has_place { PickWords::MOVING } else { first }
    }

    /// The document title's first part in the pick mode: "Placing Task".
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
