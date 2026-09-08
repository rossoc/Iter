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
//!
//! Descriptions are the exception, and that's what `--body` (the `body`
//! argument threaded through both planners) is for. A task's description
//! and its issue's body are both free-form prose that either side may have
//! edited, so there's no telling which is newer -- copying one over the
//! other happens only because you asked for it by name, and the direction
//! you ran decides which way it goes.

use crate::github::{Issue, IssueState, status_for_issue_state};
use crate::models::{Task, TaskStatus};

/// What `pull` does about one of the repo's issues. `task` is an index into
/// the `tasks` the plan was made against, rather than a borrow of one, so
/// the caller stays free to write the row it names -- and to keep its own
/// copy in step, which the next issue's plan is then made against.
#[derive(Debug, PartialEq, Eq)]
pub enum Pull {
    /// Nothing here tracks this issue, and no task shares its title: it
    /// becomes a new task, at `status`. A new task always takes the issue's
    /// body as its description, `--body` or not -- there's nothing of its
    /// own to overwrite yet.
    Create { status: TaskStatus },

    /// An existing task to write back, and what about it changes. At least
    /// one of the three is always set; when none would be, the plan is
    /// `Unchanged` instead.
    Update {
        task: usize,
        /// The task doesn't track this issue yet: it carries the issue's
        /// exact title but no issue number of its own -- the same work,
        /// entered on both sides separately. Linking the two beats creating
        /// a second task by that name (which the `UNIQUE (project_id, name)`
        /// index would refuse anyway).
        link: bool,
        /// Set when the issue's open/closed state says the status has to
        /// change.
        status: Option<TaskStatus>,
        /// Set when `--body` was asked for and the two differ.
        description: bool,
    },

    /// Tracked, and already saying the same thing on both sides.
    Unchanged,

    /// A task carries this issue's title but tracks a *different* issue.
    /// Neither creating nor adopting is right, so the issue is reported and
    /// left alone. Two issues sharing one title land here too, the second
    /// of them against the task the first just made.
    Conflict { task: usize },
}

/// What `pull` should do about `issue`, given the project's tasks and
/// whether `--body` was asked for.
///
/// The issue number is the identity that matters -- a task's name is
/// editable and an issue's title is editable, independently -- so a link,
/// once made, survives either being renamed. The title is only consulted
/// for an issue no task has claimed yet.
pub fn plan_pull(tasks: &[Task], issue: &Issue, body: bool) -> Pull {
    let tracking = tasks
        .iter()
        .position(|t| t.github_issue == Some(issue.number));
    if let Some(index) = tracking {
        return plan_update(tasks, index, issue, false, body);
    }
    match tasks.iter().position(|t| t.name == issue.title) {
        Some(index) if tasks[index].github_issue.is_some() => Pull::Conflict { task: index },
        Some(index) => plan_update(tasks, index, issue, true, body),
        // A brand-new task starts from `queue`, so a closed issue arrives
        // as `done` and an open one as `queue`.
        None => Pull::Create {
            status: status_for_issue_state(TaskStatus::Queue, issue.state),
        },
    }
}

/// The plan for an issue that has a task: what -- if anything -- the issue
/// says should change about it. Shared by the tracked case and the adopted
/// one, which differ only in whether the link itself is new.
fn plan_update(tasks: &[Task], index: usize, issue: &Issue, link: bool, body: bool) -> Pull {
    let task = &tasks[index];
    let status = status_for_issue_state(task.status, issue.state);
    let status = (status != task.status).then_some(status);
    let description = body && task.description != issue.body;
    match link || status.is_some() || description {
        false => Pull::Unchanged,
        true => Pull::Update {
            task: index,
            link,
            status,
            description,
        },
    }
}

/// What `push` does about one of the project's tasks.
#[derive(Debug, PartialEq, Eq)]
pub enum Push {
    /// The task tracks no issue: open one for it. `state` is what that
    /// issue has to end up in -- a `done` task's new issue is opened (there
    /// is no other way to make one) and then closed. A new issue always
    /// takes the task's description as its body, `--body` or not.
    Create { state: IssueState },

    /// An existing issue to edit, and what about it changes. At least one
    /// of the two is always set; when neither would be, the plan is
    /// `Unchanged` instead.
    Update {
        number: i64,
        /// Set when the issue is in the wrong state for its task's status.
        state: Option<IssueState>,
        /// Set when `--body` was asked for and the two differ.
        body: bool,
    },

    /// Tracked, and already saying the same thing on both sides.
    Unchanged,

    /// The task names an issue this repo hasn't got -- deleted, transferred,
    /// a pull request, or simply beyond the fetch limit. Opening a second
    /// issue would orphan the link, so the task is reported and left alone.
    Missing { number: i64 },
}

impl Push {
    /// Whether carrying this plan out closes an issue that wasn't closed
    /// already -- the moment the pusher signs it by adding themselves to
    /// its assignees. `Create` counts: an issue opened for a task that's
    /// already `done` is closed on the spot.
    pub fn closes(&self) -> bool {
        matches!(
            self,
            Push::Create {
                state: IssueState::Closed
            } | Push::Update {
                state: Some(IssueState::Closed),
                ..
            }
        )
    }
}

/// What `push` should do about `task`, given every issue in the repo and
/// whether `--body` was asked for.
pub fn plan_push(task: &Task, issues: &[Issue], body: bool) -> Push {
    let state = IssueState::for_status(task.status);
    let Some(number) = task.github_issue else {
        return Push::Create { state };
    };
    let Some(issue) = issues.iter().find(|i| i.number == number) else {
        return Push::Missing { number };
    };
    let state = (issue.state != state).then_some(state);
    let body = body && issue.body != task.description;
    match state.is_some() || body {
        false => Push::Unchanged,
        true => Push::Update {
            number,
            state,
            body,
        },
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

    /// The status-only `Update` a tracked issue produces, which most of the
    /// cases below expect.
    fn restatus(task: usize, status: TaskStatus) -> Pull {
        Pull::Update {
            task,
            link: false,
            status: Some(status),
            description: false,
        }
    }

    // ---- pull ------------------------------------------------------------

    #[test]
    fn an_unknown_open_issue_becomes_a_queued_task() {
        assert_eq!(
            plan_pull(&[], &issue(1, "new work", IssueState::Open), false),
            Pull::Create {
                status: TaskStatus::Queue
            }
        );
    }

    #[test]
    fn an_unknown_closed_issue_arrives_already_done() {
        assert_eq!(
            plan_pull(&[], &issue(1, "old work", IssueState::Closed), false),
            Pull::Create {
                status: TaskStatus::Done
            }
        );
    }

    #[test]
    fn a_tracked_issue_closed_upstream_marks_its_task_done() {
        let tasks = [task("login", Some(4), TaskStatus::Wip)];
        assert_eq!(
            plan_pull(&tasks, &issue(4, "login", IssueState::Closed), false),
            restatus(0, TaskStatus::Done)
        );
    }

    /// The case the whole `done -> queue` rule exists for: an issue
    /// reopened on GitHub puts its task back where `iter session new` can
    /// take it.
    #[test]
    fn a_tracked_issue_reopened_upstream_requeues_its_done_task() {
        let tasks = [task("login", Some(4), TaskStatus::Done)];
        assert_eq!(
            plan_pull(&tasks, &issue(4, "login", IssueState::Open), false),
            restatus(0, TaskStatus::Queue)
        );
    }

    #[test]
    fn a_tracked_issue_that_already_agrees_is_left_alone() {
        let tasks = [task("login", Some(4), TaskStatus::Wip)];
        assert_eq!(
            plan_pull(&tasks, &issue(4, "login", IssueState::Open), false),
            Pull::Unchanged
        );
    }

    /// A link outlives a rename on either side -- the number is the
    /// identity, so a retitled issue is still the same task.
    #[test]
    fn a_retitled_issue_is_still_matched_by_number() {
        let tasks = [task("login", Some(4), TaskStatus::Queue)];
        assert_eq!(
            plan_pull(
                &tasks,
                &issue(4, "renamed upstream", IssueState::Open),
                false
            ),
            Pull::Unchanged
        );
    }

    #[test]
    fn an_unlinked_task_with_the_issues_title_is_adopted() {
        let tasks = [task("login", None, TaskStatus::Wip)];
        assert_eq!(
            plan_pull(&tasks, &issue(4, "login", IssueState::Open), false),
            Pull::Update {
                task: 0,
                link: true,
                status: None,
                description: false,
            }
        );
    }

    /// Adopting takes the issue's state too, the same as any other pull.
    #[test]
    fn adopting_a_task_takes_the_issues_state_with_it() {
        let tasks = [task("login", None, TaskStatus::Wip)];
        assert_eq!(
            plan_pull(&tasks, &issue(4, "login", IssueState::Closed), false),
            Pull::Update {
                task: 0,
                link: true,
                status: Some(TaskStatus::Done),
                description: false,
            }
        );
    }

    /// Creating would collide on `UNIQUE (project_id, name)` and adopting
    /// would steal a task that's already spoken for, so neither happens.
    #[test]
    fn a_title_clash_with_an_otherwise_linked_task_is_a_conflict() {
        let tasks = [task("login", Some(9), TaskStatus::Queue)];
        assert_eq!(
            plan_pull(&tasks, &issue(4, "login", IssueState::Open), false),
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
            plan_pull(&tasks, &issue(2, "duplicate", IssueState::Open), false),
            Pull::Conflict { task: 0 }
        );
    }

    // ---- pull --body -----------------------------------------------------

    /// Without `--body` a differing issue body is simply not looked at:
    /// a description is prose the local side may have been editing, and
    /// nothing but an explicit ask overwrites it.
    #[test]
    fn a_differing_body_is_ignored_unless_asked_for() {
        let tasks = [task("login", Some(4), TaskStatus::Wip)];
        let mut upstream = issue(4, "login", IssueState::Open);
        upstream.body = "rewritten on github".to_string();
        assert_eq!(plan_pull(&tasks, &upstream, false), Pull::Unchanged);
        assert_eq!(
            plan_pull(&tasks, &upstream, true),
            Pull::Update {
                task: 0,
                link: false,
                status: None,
                description: true,
            }
        );
    }

    /// `--body` on a body that already matches is still nothing to do --
    /// the flag asks for agreement, not for a write every time.
    #[test]
    fn an_identical_body_is_not_a_change() {
        let tasks = [task("login", Some(4), TaskStatus::Wip)];
        assert_eq!(
            plan_pull(&tasks, &issue(4, "login", IssueState::Open), true),
            Pull::Unchanged
        );
    }

    /// State and body travel together when both differ, so one task is
    /// written once rather than twice.
    #[test]
    fn a_status_change_and_a_body_change_are_one_update() {
        let tasks = [task("login", Some(4), TaskStatus::Wip)];
        let mut upstream = issue(4, "login", IssueState::Closed);
        upstream.body = "rewritten on github".to_string();
        assert_eq!(
            plan_pull(&tasks, &upstream, true),
            Pull::Update {
                task: 0,
                link: false,
                status: Some(TaskStatus::Done),
                description: true,
            }
        );
    }

    // ---- push ------------------------------------------------------------

    #[test]
    fn an_unlinked_task_wants_an_issue_opening() {
        assert_eq!(
            plan_push(&task("login", None, TaskStatus::Wip), &[], false),
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
            plan_push(&task("shipped", None, TaskStatus::Done), &[], false),
            Push::Create {
                state: IssueState::Closed
            }
        );
    }

    #[test]
    fn a_done_task_closes_its_open_issue() {
        let issues = [issue(4, "login", IssueState::Open)];
        assert_eq!(
            plan_push(&task("login", Some(4), TaskStatus::Done), &issues, false),
            Push::Update {
                number: 4,
                state: Some(IssueState::Closed),
                body: false,
            }
        );
    }

    #[test]
    fn a_requeued_task_reopens_its_closed_issue() {
        let issues = [issue(4, "login", IssueState::Closed)];
        assert_eq!(
            plan_push(&task("login", Some(4), TaskStatus::Queue), &issues, false),
            Push::Update {
                number: 4,
                state: Some(IssueState::Open),
                body: false,
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
                plan_push(&task("login", Some(4), status), &issues, false),
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
            plan_push(&task("gone", Some(99), TaskStatus::Wip), &issues, false),
            Push::Missing { number: 99 }
        );
    }

    // ---- push --body -----------------------------------------------------

    /// The mirror of the pull case: an issue body is prose other people
    /// edit, and a status sync doesn't get to overwrite it uninvited.
    #[test]
    fn a_differing_issue_body_is_only_rewritten_when_asked_for() {
        let mut local = task("login", Some(4), TaskStatus::Wip);
        local.description = "notes from the work".to_string();
        let issues = [issue(4, "login", IssueState::Open)];
        assert_eq!(plan_push(&local, &issues, false), Push::Unchanged);
        assert_eq!(
            plan_push(&local, &issues, true),
            Push::Update {
                number: 4,
                state: None,
                body: true,
            }
        );
    }

    // ---- signing ---------------------------------------------------------

    /// The plans that close an issue are the ones that sign it, and they
    /// are the only ones: reopening doesn't, and neither does a body-only
    /// edit or an issue that was closed before this push ran.
    #[test]
    fn only_the_plans_that_close_an_issue_sign_it() {
        assert!(
            Push::Create {
                state: IssueState::Closed
            }
            .closes()
        );
        assert!(
            Push::Update {
                number: 4,
                state: Some(IssueState::Closed),
                body: false,
            }
            .closes()
        );

        assert!(
            !Push::Create {
                state: IssueState::Open
            }
            .closes()
        );
        assert!(
            !Push::Update {
                number: 4,
                state: Some(IssueState::Open),
                body: false,
            }
            .closes()
        );
        assert!(
            !Push::Update {
                number: 4,
                state: None,
                body: true,
            }
            .closes()
        );
        assert!(!Push::Unchanged.closes());
        assert!(!Push::Missing { number: 4 }.closes());
    }
}
