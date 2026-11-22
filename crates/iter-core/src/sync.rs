//! What `iter task pull` and `iter task push` decide to do about each GitHub issue and task, as plain data (`gh` calls and database writes are `iter-cli`'s `sync`).
//! Deliberately asymmetric: pulling can create a task but never touches an issue; pushing can create or close/reopen an issue but never changes a task's status, so a status cannot ping-pong.
//! Descriptions are the exception: either side may have been edited, so `--body` copies one over the other only on request, in the direction run.

use std::collections::HashMap;
use std::ops::Index;

use crate::github::{Issue, IssueState, status_for_issue_state};
use crate::models::{Task, TaskStatus};

/// A project's tasks indexed by issue number, and by title for those that track no issue yet, so each issue is planned with two lookups. The caller keeps it current as the pull goes ([`Tasks::push`], [`Tasks::link`]).
#[derive(Debug, Default)]
pub struct Tasks {
    tasks: Vec<Task>,
    by_issue: HashMap<i64, usize>,
    /// Per title, the tasks that track no issue (titles repeat; each links to at most one issue).
    unlinked: HashMap<String, Vec<usize>>,
}

impl Tasks {
    pub fn new(tasks: Vec<Task>) -> Self {
        let mut indexed = Self::default();
        for task in tasks {
            indexed.push(task);
        }
        indexed
    }

    /// Adds `task` under its issue, or under its title while it has none; a number already taken keeps pointing at the first task.
    pub fn push(&mut self, task: Task) {
        let index = self.tasks.len();
        match task.github_issue {
            Some(number) => {
                self.by_issue.entry(number).or_insert(index);
            }
            None => self
                .unlinked
                .entry(task.name.clone())
                .or_default()
                .push(index),
        }
        self.tasks.push(task);
    }

    /// Indexes the task at `index` under `issue`, which the caller is linking it to, and hands the task back for that write.
    pub fn link(&mut self, index: usize, issue: i64) -> &mut Task {
        self.by_issue.entry(issue).or_insert(index);
        let task = &mut self.tasks[index];
        if let Some(waiting) = self.unlinked.get_mut(&task.name) {
            waiting.retain(|&i| i != index);
        }
        task
    }
}

impl Index<usize> for Tasks {
    type Output = Task;

    fn index(&self, index: usize) -> &Task {
        &self.tasks[index]
    }
}

/// What `pull` does about one issue. `task` is an index into the `tasks` planned against, not a borrow, so the caller can write the row and keep its copy in step.
#[derive(Debug, PartialEq, Eq)]
pub enum Pull {
    /// Nothing tracks this issue and no single unlinked task has its title: it becomes a new task at `status`, taking the issue body as description.
    Create {
        status: TaskStatus,
    },

    /// An existing task to write back; at least one of the three is set, else `Unchanged`.
    Update {
        task: usize,
        /// The one task with the issue's exact title and no issue number (the same work entered on both sides): linking beats creating a duplicate.
        link: bool,
        /// Set when the issue's open/closed state changes the status.
        status: Option<TaskStatus>,
        /// Set when `--body` was asked for and the two differ.
        description: bool,
    },

    Unchanged,
}

/// What `pull` should do about `issue`. The issue number is the identity (names and titles are editable), so a link survives renames; the title is consulted only for an issue no task has claimed, and only when it names one unlinked task.
pub fn plan_pull(tasks: &Tasks, issue: &Issue, body: bool) -> Pull {
    if let Some(&index) = tasks.by_issue.get(&issue.number) {
        return plan_update(tasks, index, issue, false, body);
    }
    match tasks.unlinked.get(&issue.title).map(Vec::as_slice) {
        Some(&[index]) => plan_update(tasks, index, issue, true, body),
        // A brand-new task starts from `queue`, so a closed issue arrives
        // as `done` and an open one as `queue`.
        _ => Pull::Create {
            status: status_for_issue_state(TaskStatus::Queue, issue.state),
        },
    }
}

/// The plan for an issue that has a task; shared by the tracked and adopted cases.
fn plan_update(tasks: &Tasks, index: usize, issue: &Issue, link: bool, body: bool) -> Pull {
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

#[derive(Debug, PartialEq, Eq)]
pub enum PushAction {
    /// The task tracks no issue: open one, ending in `state` (a `done` task's new issue is opened then closed), with the task's description as body.
    Create {
        state: IssueState,
    },

    /// An existing issue to edit; at least one of the two is set, else `Unchanged`.
    Update {
        number: i64,
        state: Option<IssueState>,
        /// Set when `--body` was asked for and the two differ.
        body: bool,
    },

    Unchanged,

    /// The task names an issue the repo lacks (deleted, transferred, a pull request, beyond the fetch limit): reported and left alone, not re-created.
    Missing {
        number: i64,
    },
}

pub fn plan_push(task: &Task, issues: &HashMap<i64, Issue>, body: bool) -> PushAction {
    let state = IssueState::for_status(task.status);
    let Some(number) = task.github_issue else {
        return PushAction::Create { state };
    };
    let Some(issue) = issues.get(&number) else {
        return PushAction::Missing { number };
    };
    let state = (issue.state != state).then_some(state);
    let body = body && issue.body != task.description;
    match state.is_some() || body {
        false => PushAction::Unchanged,
        true => PushAction::Update {
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
            github_issue,
            status,
            ..crate::fixtures::task(1, name)
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

    fn restatus(task: usize, status: TaskStatus) -> Pull {
        Pull::Update {
            task,
            link: false,
            status: Some(status),
            description: false,
        }
    }

    #[test]
    fn an_unknown_open_issue_becomes_a_queued_task() {
        assert_eq!(
            plan_pull(
                &Tasks::default(),
                &issue(1, "new work", IssueState::Open),
                false
            ),
            Pull::Create {
                status: TaskStatus::Queue
            }
        );
    }

    #[test]
    fn an_unknown_closed_issue_arrives_already_done() {
        assert_eq!(
            plan_pull(
                &Tasks::default(),
                &issue(1, "old work", IssueState::Closed),
                false
            ),
            Pull::Create {
                status: TaskStatus::Done
            }
        );
    }

    #[test]
    fn a_tracked_issue_closed_upstream_marks_its_task_done() {
        let tasks = Tasks::new(vec![task("login", Some(4), TaskStatus::Wip)]);
        assert_eq!(
            plan_pull(&tasks, &issue(4, "login", IssueState::Closed), false),
            restatus(0, TaskStatus::Done)
        );
    }

    /// An issue reopened on GitHub puts its task back to `queue`.
    #[test]
    fn a_tracked_issue_reopened_upstream_requeues_its_done_task() {
        let tasks = Tasks::new(vec![task("login", Some(4), TaskStatus::Done)]);
        assert_eq!(
            plan_pull(&tasks, &issue(4, "login", IssueState::Open), false),
            restatus(0, TaskStatus::Queue)
        );
    }

    #[test]
    fn a_tracked_issue_that_already_agrees_is_left_alone() {
        let tasks = Tasks::new(vec![task("login", Some(4), TaskStatus::Wip)]);
        assert_eq!(
            plan_pull(&tasks, &issue(4, "login", IssueState::Open), false),
            Pull::Unchanged
        );
    }

    #[test]
    fn a_retitled_issue_is_still_matched_by_number() {
        let tasks = Tasks::new(vec![task("login", Some(4), TaskStatus::Queue)]);
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
        let tasks = Tasks::new(vec![task("login", None, TaskStatus::Wip)]);
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

    #[test]
    fn adopting_a_task_takes_the_issues_state_with_it() {
        let tasks = Tasks::new(vec![task("login", None, TaskStatus::Wip)]);
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

    /// R4: titles repeat, so a task that tracks another issue is no obstacle: the issue becomes a
    /// task of its own. (It was a `Conflict` while a project's task titles were unique.)
    #[test]
    fn an_issue_whose_title_a_linked_task_has_becomes_a_new_task() {
        let tasks = Tasks::new(vec![task("login", Some(9), TaskStatus::Queue)]);
        assert_eq!(
            plan_pull(&tasks, &issue(4, "login", IssueState::Open), false),
            Pull::Create {
                status: TaskStatus::Queue
            }
        );
    }

    /// R4: two issues sharing a title give two tasks: the first links the unlinked task of that
    /// title, the second finds none left and creates.
    #[test]
    fn a_second_issue_by_the_same_title_creates_a_task() {
        let mut tasks = Tasks::new(vec![task("duplicate", None, TaskStatus::Queue)]);
        let first = issue(1, "duplicate", IssueState::Open);
        assert!(matches!(
            plan_pull(&tasks, &first, false),
            Pull::Update {
                task: 0,
                link: true,
                ..
            }
        ));
        tasks.link(0, first.number);
        assert_eq!(
            plan_pull(&tasks, &issue(2, "duplicate", IssueState::Open), false),
            Pull::Create {
                status: TaskStatus::Queue
            }
        );
    }

    /// R4: an issue is linked to an unlinked task only when exactly one has its title; with two
    /// (or after the first of several issues took one) the choice is not ours: a new task.
    #[test]
    fn an_issue_is_linked_only_to_the_one_unlinked_task_of_its_title() {
        let tasks = Tasks::new(vec![
            task("login", None, TaskStatus::Queue),
            task("login", None, TaskStatus::Queue),
        ]);
        assert_eq!(
            plan_pull(&tasks, &issue(4, "login", IssueState::Open), false),
            Pull::Create {
                status: TaskStatus::Queue
            }
        );
    }

    #[test]
    fn a_differing_body_is_ignored_unless_asked_for() {
        let tasks = Tasks::new(vec![task("login", Some(4), TaskStatus::Wip)]);
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

    #[test]
    fn an_identical_body_is_not_a_change() {
        let tasks = Tasks::new(vec![task("login", Some(4), TaskStatus::Wip)]);
        assert_eq!(
            plan_pull(&tasks, &issue(4, "login", IssueState::Open), true),
            Pull::Unchanged
        );
    }

    #[test]
    fn a_status_change_and_a_body_change_are_one_update() {
        let tasks = Tasks::new(vec![task("login", Some(4), TaskStatus::Wip)]);
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

    #[test]
    fn an_unlinked_task_wants_an_issue_opening() {
        assert_eq!(
            plan_push(
                &task("login", None, TaskStatus::Wip),
                &HashMap::new(),
                false
            ),
            PushAction::Create {
                state: IssueState::Open
            }
        );
    }

    #[test]
    fn an_unlinked_done_task_gets_an_issue_that_ends_up_closed() {
        assert_eq!(
            plan_push(
                &task("shipped", None, TaskStatus::Done),
                &HashMap::new(),
                false
            ),
            PushAction::Create {
                state: IssueState::Closed
            }
        );
    }

    #[test]
    fn a_done_task_closes_its_open_issue() {
        let issues = HashMap::from([(4, issue(4, "login", IssueState::Open))]);
        assert_eq!(
            plan_push(&task("login", Some(4), TaskStatus::Done), &issues, false),
            PushAction::Update {
                number: 4,
                state: Some(IssueState::Closed),
                body: false,
            }
        );
    }

    #[test]
    fn a_requeued_task_reopens_its_closed_issue() {
        let issues = HashMap::from([(4, issue(4, "login", IssueState::Closed))]);
        assert_eq!(
            plan_push(&task("login", Some(4), TaskStatus::Queue), &issues, false),
            PushAction::Update {
                number: 4,
                state: Some(IssueState::Open),
                body: false,
            }
        );
    }

    /// `wip` is open work like `queue`: nothing to tell GitHub.
    #[test]
    fn wip_and_queue_both_leave_an_open_issue_open() {
        let issues = HashMap::from([(4, issue(4, "login", IssueState::Open))]);
        for status in [TaskStatus::Queue, TaskStatus::Wip] {
            assert_eq!(
                plan_push(&task("login", Some(4), status), &issues, false),
                PushAction::Unchanged
            );
        }
    }

    #[test]
    fn a_link_to_an_issue_the_repo_hasnt_got_is_reported_not_recreated() {
        let issues = HashMap::from([(4, issue(4, "login", IssueState::Open))]);
        assert_eq!(
            plan_push(&task("gone", Some(99), TaskStatus::Wip), &issues, false),
            PushAction::Missing { number: 99 }
        );
    }

    #[test]
    fn a_differing_issue_body_is_only_rewritten_when_asked_for() {
        let mut local = task("login", Some(4), TaskStatus::Wip);
        local.description = "notes from the work".to_string();
        let issues = HashMap::from([(4, issue(4, "login", IssueState::Open))]);
        assert_eq!(plan_push(&local, &issues, false), PushAction::Unchanged);
        assert_eq!(
            plan_push(&local, &issues, true),
            PushAction::Update {
                number: 4,
                state: None,
                body: true,
            }
        );
    }
}
