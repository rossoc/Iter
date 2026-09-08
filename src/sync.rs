//! Reconciling a project's tasks with its GitHub issues: what `iter task
//! pull` and `iter task push` decide to do about each one.
//!
//! The decisions live here, as plain data, so they can be tested without a
//! `gh` in sight; carrying them out -- the `gh` calls and the database
//! writes -- is `main`'s half. The two directions are deliberately not
//! symmetrical: pulling can *create a task* but never touches an issue, and
//! pushing can *create or close/reopen an issue* but never changes a task's
//! status. Each side is the authority on what it owns, so running one after
//! the other can't ping-pong a task between two statuses.

use crate::github::{Issue, IssueState, status_for_issue_state};
use crate::models::{Task, TaskStatus};

/// What `pull` does about one of the repo's issues. `task` is an index into
/// the `tasks` the plan was made against, rather than a borrow of one, so
/// the caller stays free to write the row it names -- and to keep its own
/// copy in step, which the next issue's plan is then made against.
#[derive(Debug, PartialEq, Eq)]
pub enum Pull {
    /// Nothing here tracks this issue, and no task shares its title:
    /// it becomes a new task, at `status`.
    Create { status: TaskStatus },

    /// A task tracks this issue and the issue's state says its status has
    /// to change.
    Restatus { task: usize, status: TaskStatus },

    /// A task carries this issue's exact title but tracks no issue at all
    /// -- the same work, entered on both sides separately. Link the two
    /// rather than creating a second task by that name (which the
    /// `UNIQUE (project_id, name)` index would refuse anyway), and take the
    /// issue's state while we're there.
    Adopt { task: usize, status: TaskStatus },

    /// Tracked, and already saying the same thing on both sides.
    Unchanged,

    /// A task carries this issue's title but tracks a *different* issue.
    /// Neither creating nor adopting is right, so the issue is reported and
    /// left alone. Two issues sharing one title land here too, the second
    /// of them against the task the first just made.
    Conflict { task: usize },
}

/// What `pull` should do about `issue`, given the project's tasks.
///
/// The issue number is the identity that matters -- a task's name is
/// editable and an issue's title is editable, independently -- so a link,
/// once made, survives either being renamed. The title is only consulted
/// for an issue no task has claimed yet.
pub fn plan_pull(tasks: &[Task], issue: &Issue) -> Pull {
    let tracking = tasks
        .iter()
        .position(|t| t.github_issue == Some(issue.number));
    if let Some(index) = tracking {
        let task = &tasks[index];
        let status = status_for_issue_state(task.status, issue.state);
        return match status == task.status {
            true => Pull::Unchanged,
            false => Pull::Restatus {
                task: index,
                status,
            },
        };
    }
    match tasks.iter().position(|t| t.name == issue.title) {
        Some(index) if tasks[index].github_issue.is_some() => Pull::Conflict { task: index },
        Some(index) => Pull::Adopt {
            task: index,
            status: status_for_issue_state(tasks[index].status, issue.state),
        },
        // A brand-new task starts from `queue`, so a closed issue arrives
        // as `done` and an open one as `queue`.
        None => Pull::Create {
            status: status_for_issue_state(TaskStatus::Queue, issue.state),
        },
    }
}

/// What `push` does about one of the project's tasks.
#[derive(Debug, PartialEq, Eq)]
pub enum Push {
    /// The task tracks no issue: open one for it. `state` is what that
    /// issue has to end up in -- a `done` task's new issue is opened (there
    /// is no other way to make one) and then closed.
    Create { state: IssueState },

    /// The task's issue is in the wrong state for its status.
    Restate { number: i64, state: IssueState },

    /// Tracked, and already saying the same thing on both sides.
    Unchanged,

    /// The task names an issue this repo hasn't got -- deleted, transferred,
    /// a pull request, or simply beyond the fetch limit. Opening a second
    /// issue would orphan the link, so the task is reported and left alone.
    Missing { number: i64 },
}

/// What `push` should do about `task`, given every issue in the repo.
pub fn plan_push(task: &Task, issues: &[Issue]) -> Push {
    let state = IssueState::for_status(task.status);
    let Some(number) = task.github_issue else {
        return Push::Create { state };
    };
    match issues.iter().find(|i| i.number == number) {
        None => Push::Missing { number },
        Some(issue) if issue.state == state => Push::Unchanged,
        Some(_) => Push::Restate { number, state },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(name: &str, github_issue: Option<i64>, status: TaskStatus) -> Task {
        Task {
            id: Some(1),
            project_id: 1,
            name: name.to_string(),
            description: String::new(),
            github_issue,
            status,
            branch_prefix: String::new(),
        }
    }

    fn issue(number: i64, title: &str, state: IssueState) -> Issue {
        Issue {
            number,
            title: title.to_string(),
            body: String::new(),
            state,
        }
    }

    // ---- pull ------------------------------------------------------------

    #[test]
    fn an_unknown_open_issue_becomes_a_queued_task() {
        assert_eq!(
            plan_pull(&[], &issue(1, "new work", IssueState::Open)),
            Pull::Create {
                status: TaskStatus::Queue
            }
        );
    }

    #[test]
    fn an_unknown_closed_issue_arrives_already_done() {
        assert_eq!(
            plan_pull(&[], &issue(1, "old work", IssueState::Closed)),
            Pull::Create {
                status: TaskStatus::Done
            }
        );
    }

    #[test]
    fn a_tracked_issue_closed_upstream_marks_its_task_done() {
        let tasks = [task("login", Some(4), TaskStatus::Wip)];
        assert_eq!(
            plan_pull(&tasks, &issue(4, "login", IssueState::Closed)),
            Pull::Restatus {
                task: 0,
                status: TaskStatus::Done
            }
        );
    }

    /// The case the whole `done -> queue` rule exists for: an issue
    /// reopened on GitHub puts its task back where `iter session new` can
    /// take it.
    #[test]
    fn a_tracked_issue_reopened_upstream_requeues_its_done_task() {
        let tasks = [task("login", Some(4), TaskStatus::Done)];
        assert_eq!(
            plan_pull(&tasks, &issue(4, "login", IssueState::Open)),
            Pull::Restatus {
                task: 0,
                status: TaskStatus::Queue
            }
        );
    }

    #[test]
    fn a_tracked_issue_that_already_agrees_is_left_alone() {
        let tasks = [task("login", Some(4), TaskStatus::Wip)];
        assert_eq!(
            plan_pull(&tasks, &issue(4, "login", IssueState::Open)),
            Pull::Unchanged
        );
    }

    /// A link outlives a rename on either side -- the number is the
    /// identity, so a retitled issue is still the same task.
    #[test]
    fn a_retitled_issue_is_still_matched_by_number() {
        let tasks = [task("login", Some(4), TaskStatus::Queue)];
        assert_eq!(
            plan_pull(&tasks, &issue(4, "renamed upstream", IssueState::Open)),
            Pull::Unchanged
        );
    }

    #[test]
    fn an_unlinked_task_with_the_issues_title_is_adopted() {
        let tasks = [task("login", None, TaskStatus::Wip)];
        assert_eq!(
            plan_pull(&tasks, &issue(4, "login", IssueState::Open)),
            Pull::Adopt {
                task: 0,
                status: TaskStatus::Wip
            }
        );
    }

    /// Adopting takes the issue's state too, the same as any other pull.
    #[test]
    fn adopting_a_task_takes_the_issues_state_with_it() {
        let tasks = [task("login", None, TaskStatus::Wip)];
        assert_eq!(
            plan_pull(&tasks, &issue(4, "login", IssueState::Closed)),
            Pull::Adopt {
                task: 0,
                status: TaskStatus::Done
            }
        );
    }

    /// Creating would collide on `UNIQUE (project_id, name)` and adopting
    /// would steal a task that's already spoken for, so neither happens.
    #[test]
    fn a_title_clash_with_an_otherwise_linked_task_is_a_conflict() {
        let tasks = [task("login", Some(9), TaskStatus::Queue)];
        assert_eq!(
            plan_pull(&tasks, &issue(4, "login", IssueState::Open)),
            Pull::Conflict { task: 0 }
        );
    }

    /// Two issues can share a title. The first becomes a task; the second
    /// has to be seen as the clash it is, rather than the caller trying to
    /// insert a second task by that name and losing the rest of the pull to
    /// a `UNIQUE (project_id, name)` failure. The caller keeps its view of
    /// the tasks current for exactly this.
    #[test]
    fn a_second_issue_by_the_same_title_clashes_with_the_task_the_first_made() {
        let tasks = [task("duplicate", Some(1), TaskStatus::Queue)];
        assert_eq!(
            plan_pull(&tasks, &issue(2, "duplicate", IssueState::Open)),
            Pull::Conflict { task: 0 }
        );
    }

    // ---- push ------------------------------------------------------------

    #[test]
    fn an_unlinked_task_wants_an_issue_opening() {
        assert_eq!(
            plan_push(&task("login", None, TaskStatus::Wip), &[]),
            Push::Create {
                state: IssueState::Open
            }
        );
    }

    /// A `done` task still gets an issue -- there's no way to create a
    /// closed one, so it's opened and then closed.
    #[test]
    fn an_unlinked_done_task_gets_an_issue_that_ends_up_closed() {
        assert_eq!(
            plan_push(&task("shipped", None, TaskStatus::Done), &[]),
            Push::Create {
                state: IssueState::Closed
            }
        );
    }

    #[test]
    fn a_done_task_closes_its_open_issue() {
        let issues = [issue(4, "login", IssueState::Open)];
        assert_eq!(
            plan_push(&task("login", Some(4), TaskStatus::Done), &issues),
            Push::Restate {
                number: 4,
                state: IssueState::Closed
            }
        );
    }

    #[test]
    fn a_requeued_task_reopens_its_closed_issue() {
        let issues = [issue(4, "login", IssueState::Closed)];
        assert_eq!(
            plan_push(&task("login", Some(4), TaskStatus::Queue), &issues),
            Push::Restate {
                number: 4,
                state: IssueState::Open
            }
        );
    }

    /// `wip` is open work, exactly as `queue` is -- moving a task from one
    /// to the other is not something GitHub can be told, so pushing it
    /// touches nothing.
    #[test]
    fn wip_and_queue_both_leave_an_open_issue_open() {
        let issues = [issue(4, "login", IssueState::Open)];
        for status in [TaskStatus::Queue, TaskStatus::Wip] {
            assert_eq!(
                plan_push(&task("login", Some(4), status), &issues),
                Push::Unchanged
            );
        }
    }

    /// Opening a second issue would drop the link the task already has, so
    /// a number the repo doesn't know is reported, not re-created.
    #[test]
    fn a_link_to_an_issue_the_repo_hasnt_got_is_reported_not_recreated() {
        let issues = [issue(4, "login", IssueState::Open)];
        assert_eq!(
            plan_push(&task("gone", Some(99), TaskStatus::Wip), &issues),
            Push::Missing { number: 99 }
        );
    }
}
