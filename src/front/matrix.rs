//! The priority matrix as data. From `web/quadrants.rs` in isolation step 4: a [`Quadrant`] has
//! no DOM `id` (the web maps a quadrant to its element) and [`place`] takes the chosen quadrant,
//! `None` for "back to the waiting list"; the form value is parsed in the web.

use crate::models::{Priority, Task};

/// One quadrant: how it is told apart and what it means. No DOM id.
#[allow(dead_code)] // stub: I3
pub struct Quadrant {
    pub target: &'static str,
    pub flags: Priority,
    pub name: &'static str,
    pub rule: &'static str,
    pub empty: &'static str,
}

/// Places `task` in `quadrant` (its priority flags are returned for the caller to store) or, with
/// `None`, takes it back out of the matrix (no flags to store).
// stub: I3 (`web::quadrants::place` still takes the target word)
#[allow(dead_code)]
pub fn place(task: &mut Task, quadrant: Option<&Quadrant>) -> Option<Priority> {
    let _ = (task, quadrant);
    unimplemented!("stub: I3")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;

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
