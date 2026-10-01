//! The matrix's four quadrants as data, and the split of a board's cards
//! into the "Not placed" list and the quadrants. Pure functions, no markup.

use crate::models::{Card, Priority, Task};

/// One quadrant: what a drop on it posts, the flags it stands for and its
/// words. The order is the reading order (row by row: 1 2 / 3 4).
pub struct Quadrant {
    /// What `board.js` posts as the drop's target.
    pub target: &'static str,
    /// The heading's id.
    pub id: &'static str,
    pub flags: Priority,
    pub name: &'static str,
    /// What puts a task here, in words.
    pub rule: &'static str,
    /// The line shown when it holds no card.
    pub empty: &'static str,
}

const fn flags(urgent: bool, important: bool) -> Priority {
    Priority { urgent, important }
}

/// The flags of quadrant `n` (an index into [`QUADRANTS`]) and, the other way,
/// [`quadrant_index`]: the two halves of one rule, kept side by side. The
/// bits are "not urgent" (1) and "not important" (2), so the reading order
/// is: both (0), important (1), urgent (2), neither (3).
const fn flags_at(n: usize) -> Priority {
    flags(n & 1 == 0, n & 2 == 0)
}

pub const QUADRANTS: [Quadrant; 4] = [
    Quadrant {
        target: "both",
        id: "q-both",
        flags: flags_at(0),
        name: "Do first",
        rule: "Important, urgent",
        empty: "Nothing to do first.",
    },
    Quadrant {
        target: "important",
        id: "q-important",
        flags: flags_at(1),
        name: "Plan",
        rule: "Important, not urgent",
        empty: "Nothing to plan.",
    },
    Quadrant {
        target: "urgent",
        id: "q-urgent",
        flags: flags_at(2),
        name: "Delegate",
        rule: "Not important, urgent",
        empty: "Nothing to delegate.",
    },
    Quadrant {
        target: "neither",
        id: "q-neither",
        flags: flags_at(3),
        name: "Eliminate",
        rule: "Not important, not urgent",
        empty: "Nothing to eliminate.",
    },
];

/// The quadrant a priority belongs to (an index into [`QUADRANTS`]): the
/// two flags are the two bits, no scan of the table.
pub fn quadrant_index(priority: Priority) -> usize {
    usize::from(!priority.urgent) | usize::from(!priority.important) << 1
}

/// A board's unfinished cards: those not placed yet and, for each quadrant,
/// those placed in it.
#[derive(Default)]
pub struct Matrix {
    pub waiting: Vec<Card>,
    pub placed: [Vec<Card>; 4],
}

impl Matrix {
    /// No card at all: the board has no unfinished task.
    pub fn is_empty(&self) -> bool {
        self.waiting.is_empty() && self.placed.iter().all(Vec::is_empty)
    }
}

/// Sorts `cards` (already in list order) in one pass: a placed card goes to
/// the quadrant its flags say, any other to the waiting list. Each keeps its
/// place in the list order.
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

/// Applies a matrix drop. `target` is `left` (back to the side list, flags
/// untouched) or a quadrant, whose flags the task then gets.
pub fn place(task: &mut Task, target: &str) -> Result<Option<Priority>, String> {
    if target == "left" {
        task.matrix_placed = false;
        return Ok(None);
    }
    let quadrant = QUADRANTS
        .iter()
        .find(|q| q.target == target)
        .ok_or_else(|| format!("unknown quadrant '{target}'"))?;
    task.matrix_placed = true;
    Ok(Some(quadrant.flags))
}

#[cfg(test)]
mod tests {
    use super::*;
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

    #[test]
    fn quadrants_set_the_flags_and_the_side_list_only_unplaces() {
        let mut t = Task::template(1, String::new(), TaskStatus::Queue);
        assert_eq!(place(&mut t, "both"), Ok(Some(flags(true, true))));
        assert!(t.matrix_placed);
        assert_eq!(place(&mut t, "urgent"), Ok(Some(flags(true, false))));
        assert_eq!(place(&mut t, "left"), Ok(None));
        assert!(!t.matrix_placed);
        assert_eq!(place(&mut t, "neither"), Ok(Some(flags(false, false))));
        assert!(t.matrix_placed);
        assert!(place(&mut t, "nowhere").is_err());
    }
}
