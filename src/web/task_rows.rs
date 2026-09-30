//! The mapping from tasks to what the task table shows (`ui::task_table`)
//! and their tags to chips (`ui::tag_chips`): the one place a task's links,
//! issue label and chips are built.

use super::ui::tag_chips::Chip;
use super::ui::task_table::{ProjectLink, TaskRow};
use super::url::{project_url, task_url};
use crate::db::Table;
use crate::models::{Tag, Task};

/// `#12`, or empty.
pub fn issue_label(issue: Option<i64>) -> String {
    issue.map(|n| format!("#{n}")).unwrap_or_default()
}

/// A chip per tag, in the tags' own colors.
pub fn chips(tags: &[Tag]) -> Vec<Chip<'_>> {
    tags.iter()
        .map(|t| Chip {
            name: &t.name,
            color: &t.color,
        })
        .collect()
}

fn row<'a>(task: &'a Task, project: Option<ProjectLink<'a>>) -> TaskRow<'a> {
    TaskRow {
        name: &task.name,
        href: task_url(task.id()),
        project,
        status: task.status,
        issue: issue_label(task.github_issue),
    }
}

/// The rows of one project's tasks: no Project column.
pub fn project_rows(tasks: &[Task]) -> Vec<TaskRow<'_>> {
    tasks.iter().map(|t| row(t, None)).collect()
}

/// The rows of a group's tasks, each with its project (the pairs
/// `Db::tasks_in` returns): with a Project column.
pub fn group_rows(tasks: &[(String, Task)]) -> Vec<TaskRow<'_>> {
    tasks
        .iter()
        .map(|(project, t)| {
            let link = ProjectLink {
                name: project,
                href: project_url(t.project_id),
            };
            row(t, Some(link))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TaskStatus;

    fn task(id: i64, project_id: i64, issue: Option<i64>) -> Task {
        let mut t = Task::template(project_id, String::new(), TaskStatus::Wip);
        t.id = Some(id);
        t.name = format!("t{id}");
        t.github_issue = issue;
        t
    }

    #[test]
    fn rows_link_the_task_and_label_the_issue() {
        let tasks = [task(1, 9, Some(12)), task(2, 9, None)];
        let rows = project_rows(&tasks);
        assert_eq!(rows[0].href, "/task/1");
        assert_eq!(
            (rows[0].issue.as_str(), rows[1].issue.as_str()),
            ("#12", "")
        );
        assert!(rows.iter().all(|r| r.project.is_none()));
    }

    #[test]
    fn chips_carry_the_tags_name_and_color() {
        let tags = [Tag {
            id: None,
            name: "Urgent".into(),
            color: "#facc15".into(),
            description: String::new(),
        }];
        let chips = chips(&tags);
        assert_eq!((chips[0].name, chips[0].color), ("Urgent", "#facc15"));
        assert!(super::chips(&[]).is_empty());
    }

    #[test]
    fn group_rows_carry_their_project() {
        let pairs = [("app".to_string(), task(3, 9, None))];
        let rows = group_rows(&pairs);
        let p = rows[0].project.as_ref().expect("project");
        assert_eq!((p.name, p.href.as_str()), ("app", "/project/9"));
    }
}
