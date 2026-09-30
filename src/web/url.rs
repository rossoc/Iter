//! The URLs of the pages, built in one place so no call site concatenates
//! a path by hand.

use crate::reporting::fmt_date;
use chrono::NaiveDate;

/// `base?k=v&k=v`, leaving out the `None`s (and the `?` when nothing is
/// left). Values are percent-encoded.
pub fn page_url(base: &str, params: &[(&str, Option<&str>)]) -> String {
    let mut url = base.to_string();
    let mut sep = if base.contains('?') { '&' } else { '?' };
    for (key, value) in params {
        if let Some(value) = value {
            url.push(sep);
            url.push_str(key);
            url.push('=');
            for b in value.bytes() {
                if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                    url.push(b as char);
                } else {
                    url.push_str(&format!("%{b:02X}"));
                }
            }
            sep = '&';
        }
    }
    url
}

/// The list of boards, and the section the top bar's Board link opens.
pub const BOARDS: &str = "/boards";

pub fn org_url(id: i64) -> String {
    format!("/org/{id}")
}

pub fn project_url(id: i64) -> String {
    format!("/project/{id}")
}

pub fn task_url(id: i64) -> String {
    format!("/task/{id}")
}

/// The form that adds a task to project `project_id`.
pub fn new_task_url(project_id: i64) -> String {
    format!("{}/task/new", project_url(project_id))
}

/// The edit form of the page at `url`.
pub fn edit_url(url: &str) -> String {
    format!("{url}/edit")
}

/// The organization's edit form.
pub fn org_edit_url(id: i64) -> String {
    edit_url(&org_url(id))
}

/// The task's edit form.
pub fn task_edit_url(id: i64) -> String {
    edit_url(&task_url(id))
}

/// The project's edit form.
pub fn project_edit_url(id: i64) -> String {
    edit_url(&project_url(id))
}

/// A board's page: its day agenda (`?date=` picks the day).
pub fn board_url(id: i64) -> String {
    format!("/board/{id}")
}

/// The agenda of `day` on board `id`.
pub fn board_day_url(id: i64, day: NaiveDate) -> String {
    page_url(&board_url(id), &[("date", Some(&fmt_date(day)))])
}

/// The board's Eisenhower matrix.
pub fn board_matrix_url(id: i64) -> String {
    format!("{}/matrix", board_url(id))
}

/// The board's description and projects.
pub fn board_info_url(id: i64) -> String {
    format!("{}/info", board_url(id))
}

/// Where a task dropped on the agenda (or scheduled with the pick forms) is
/// posted.
pub fn board_schedule_url(id: i64) -> String {
    format!("{}/schedule", board_url(id))
}

/// The board's edit form.
pub fn board_edit_url(id: i64) -> String {
    edit_url(&board_url(id))
}

/// The Tasks tab of the organization or project page at `base`.
pub fn tasks_tab_url(base: &str) -> String {
    page_url(base, &[("tab", Some("tasks"))])
}

/// The Tasks tab of project `id`: where a task's pages lead back to.
pub fn project_tasks_url(id: i64) -> String {
    tasks_tab_url(&project_url(id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_url_skips_none_and_encodes() {
        assert_eq!(page_url("/org/1", &[("tab", None)]), "/org/1");
        assert_eq!(
            page_url(
                "/org/1",
                &[("tab", Some("report")), ("from", Some("")), ("to", None)]
            ),
            "/org/1?tab=report&from="
        );
        assert_eq!(
            page_url("/a?x=1", &[("y", Some("a b&c"))]),
            "/a?x=1&y=a%20b%26c"
        );
    }

    #[test]
    fn entity_urls() {
        assert_eq!(org_url(3), "/org/3");
        assert_eq!(project_url(4), "/project/4");
        assert_eq!(task_url(5), "/task/5");
        assert_eq!(new_task_url(4), "/project/4/task/new");
        assert_eq!(edit_url(&org_url(3)), "/org/3/edit");
        assert_eq!(org_edit_url(3), "/org/3/edit");
        assert_eq!(task_edit_url(5), "/task/5/edit");
        assert_eq!(project_edit_url(4), "/project/4/edit");
        assert_eq!(board_url(2), "/board/2");
        assert_eq!(board_matrix_url(2), "/board/2/matrix");
        let day = NaiveDate::from_ymd_opt(2026, 9, 5).unwrap();
        assert_eq!(board_day_url(2, day), "/board/2?date=2026-09-05");
        assert_eq!(board_info_url(2), "/board/2/info");
        assert_eq!(board_edit_url(2), "/board/2/edit");
        assert_eq!(board_schedule_url(2), "/board/2/schedule");
        assert_eq!(tasks_tab_url("/org/3"), "/org/3?tab=tasks");
        assert_eq!(project_tasks_url(4), "/project/4?tab=tasks");
    }

    #[test]
    fn the_nav_links_to_the_boards() {
        assert_eq!(super::super::ui::frame::NAV[0].1, BOARDS);
    }
}
