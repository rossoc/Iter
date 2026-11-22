//! The priority matrix as data: its four quadrants and the split of a board's cards into the "Not
//! placed" list and the quadrants. A [`Quadrant`] has no DOM id (the web maps a quadrant to its
//! element) and [`place`] takes the chosen quadrant, `None` for "back to the waiting list"; the form
//! value is parsed in the web.

use crate::models::{Card, Priority, Task};

/// The order is the reading order (row by row: 1 2 / 3 4).
pub struct Quadrant {
    pub target: &'static str,
    pub flags: Priority,
    pub name: &'static str,
    pub rule: &'static str,
    pub empty: &'static str,
}

const fn flags(urgent: bool, important: bool) -> Priority {
    Priority { urgent, important }
}

/// The flags of quadrant `n` and, the other way, [`quadrant_index`]: two halves of one rule. The bits are "not urgent" (1) and "not important" (2).
const fn flags_at(n: usize) -> Priority {
    flags(n & 1 == 0, n & 2 == 0)
}

pub const QUADRANTS: [Quadrant; 4] = [
    Quadrant {
        target: "both",
        flags: flags_at(0),
        name: "Do first",
        rule: "Important, urgent",
        empty: "Nothing to do first.",
    },
    Quadrant {
        target: "important",
        flags: flags_at(1),
        name: "Plan",
        rule: "Important, not urgent",
        empty: "Nothing to plan.",
    },
    Quadrant {
        target: "urgent",
        flags: flags_at(2),
        name: "Delegate",
        rule: "Not important, urgent",
        empty: "Nothing to delegate.",
    },
    Quadrant {
        target: "neither",
        flags: flags_at(3),
        name: "Eliminate",
        rule: "Not important, not urgent",
        empty: "Nothing to eliminate.",
    },
];

pub fn quadrant_index(priority: Priority) -> usize {
    usize::from(!priority.urgent) | usize::from(!priority.important) << 1
}

#[derive(Default)]
pub struct Matrix {
    pub waiting: Vec<Card>,
    pub placed: [Vec<Card>; 4],
}

impl Matrix {
    pub fn is_empty(&self) -> bool {
        self.waiting.is_empty() && self.placed.iter().all(Vec::is_empty)
    }
}

pub fn split(cards: Vec<Card>) -> Matrix {
    let mut out = Matrix::default();
    for card in cards {
        if card.task.matrix_placed {
            out.placed[quadrant_index(card.priority)].push(card);
        } else {
            out.waiting.push(card);
        }
    }
    // the tasks in progress first (stable: the rest keeps its order)
    out.waiting
        .sort_by_key(|c| c.task.status != crate::models::TaskStatus::Wip);
    out
}

/// Places `task` in `quadrant` (its priority flags are returned for the caller to store) or, with
/// `None`, takes it back out of the matrix (no flags to store).
pub fn place(task: &mut Task, quadrant: Option<&Quadrant>) -> Option<Priority> {
    task.matrix_placed = quadrant.is_some();
    quadrant.map(|q| q.flags)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::models::TaskStatus;

    fn card(label: &str, placed: bool, urgent: bool, important: bool) -> Card {
        let mut task = Task::template(1, String::new(), TaskStatus::Queue);
        task.matrix_placed = placed;
        Card {
            label: label.into(),
            task,
            priority: flags(urgent, important),
        }
    }

    fn labels(cards: &[Card]) -> Vec<&str> {
        cards.iter().map(|c| c.label.as_str()).collect()
    }

    #[test]
    fn every_priority_has_its_quadrant() {
        for urgent in [false, true] {
            for important in [false, true] {
                let p = flags(urgent, important);
                assert_eq!(QUADRANTS[quadrant_index(p)].flags, p);
            }
        }
        for (n, quadrant) in QUADRANTS.iter().enumerate() {
            assert_eq!(quadrant_index(quadrant.flags), n);
        }
        let targets: Vec<_> = QUADRANTS.iter().map(|q| q.target).collect();
        assert_eq!(targets, ["both", "important", "urgent", "neither"]);
    }

    #[test]
    fn the_waiting_list_puts_work_in_progress_first() {
        let mut started = card("started", false, false, false);
        started.task.status = crate::models::TaskStatus::Wip;
        let out = split(vec![
            card("a", false, true, true),
            started,
            card("b", false, false, false),
        ]);
        assert_eq!(labels(&out.waiting), ["started", "a", "b"]);
    }

    #[test]
    fn cards_go_to_the_list_or_their_quadrant_in_order() {
        let out = split(vec![
            card("a", false, true, true),
            card("b", true, true, true),
            card("c", true, false, true),
            card("d", true, true, false),
            card("e", true, false, false),
            card("f", true, true, true),
            card("g", false, false, false),
        ]);
        assert_eq!(labels(&out.waiting), ["a", "g"]);
        assert_eq!(labels(&out.placed[0]), ["b", "f"]);
        assert_eq!(labels(&out.placed[1]), ["c"]);
        assert_eq!(labels(&out.placed[2]), ["d"]);
        assert_eq!(labels(&out.placed[3]), ["e"]);
        assert!(!out.is_empty());
    }

    #[test]
    fn a_board_without_cards_is_empty() {
        assert!(split(Vec::new()).is_empty());
        assert!(!split(vec![card("a", false, false, false)]).is_empty());
        assert!(!split(vec![card("a", true, false, false)]).is_empty());
    }

    fn quadrant(target: &'static str, urgent: bool, important: bool) -> Quadrant {
        Quadrant {
            target,
            flags: Priority { urgent, important },
            name: "",
            rule: "",
            empty: "",
        }
    }

    #[test]
    fn placing_in_a_quadrant_marks_the_task_and_returns_its_flags() {
        let mut task = fixtures::task(1, "login");
        let plan = quadrant("important", false, true);
        assert_eq!(place(&mut task, Some(&plan)), Some(plan.flags));
        assert!(task.matrix_placed);
    }

    #[test]
    fn placing_in_no_quadrant_takes_the_task_out_of_the_matrix() {
        let mut task = fixtures::task(1, "login");
        task.matrix_placed = true;
        assert_eq!(place(&mut task, None), None);
        assert!(!task.matrix_placed);
    }
}
