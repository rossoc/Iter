//! The URLs of the pages, built in one place so no call site concatenates
//! a path by hand.

use super::task_query::DEFAULT;
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

/// The global settings page, and where a save lands.
pub const SETTINGS: &str = "/settings";

pub fn settings_saved_url() -> String {
    page_url(SETTINGS, &[("saved", Some("1"))])
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

/// `url` with `side=off` (a board's side column (Unscheduled, Backlog) folded into its rail) when
/// `folded`, before the fragment: what every link of a board keeps, so the
/// column stays as it was left.
pub fn with_side(url: String, folded: bool) -> String {
    if !folded {
        return url;
    }
    let (base, fragment) = match url.split_once('#') {
        Some((base, fragment)) => (base, Some(fragment)),
        None => (url.as_str(), None),
    };
    let folded = page_url(base, &[("side", Some("off"))]);
    match fragment {
        Some(fragment) => format!("{folded}#{fragment}"),
        None => folded,
    }
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

/// The Tasks tab of the organization or project page at `base`, as every
/// link into it opens it: with the default search, spelled out
/// (`task_query::DEFAULT`), since a list with no `q` is unfiltered.
pub fn tasks_tab_url(base: &str) -> String {
    tasks_list_url(base, Some(DEFAULT))
}

/// The Tasks tab of the page at `base`, with the search `q`; none (what an
/// emptied search box sends) is no filter at all.
pub fn tasks_list_url(base: &str, q: Option<&str>) -> String {
    page_url(base, &[("tab", Some("tasks")), ("q", q)])
}

/// Where the New task pop-up of a task list posts (`task_new.rs`).
pub const NEW_TASK: &str = "/task/new";

/// The folder browser the project create row opens.
pub const FOLDERS: &str = "/folders";

/// Where New project's Choose button posts: the system's folder picker
/// (`folder_picker.rs`).
pub const PICK_FOLDER: &str = "/folders/pick";

/// The folder browser at `path`, for the list `home` (a `Home` token); `name`
/// is what was typed in the create row, `hidden` shows the folders that start
/// with a dot.
pub fn folders_url(home: &str, name: &str, path: &str, hidden: bool) -> String {
    page_url(
        FOLDERS,
        &[
            ("home", Some(home)),
            ("name", (!name.is_empty()).then_some(name)),
            ("base_path", Some(path)),
            ("hidden", hidden.then_some("1")),
        ],
    )
}

/// Where the create row of board `id`'s project list posts.
pub fn quick_board_project_url(id: i64) -> String {
    format!("{}/project/quick", board_url(id))
}

/// Where the create row of organization `id`'s project list posts.
pub fn quick_project_url(id: i64) -> String {
    format!("{}/project/quick", org_url(id))
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
        assert_eq!(tasks_tab_url("/org/3"), "/org/3?tab=tasks&q=is%3Aopen");
        assert_eq!(project_tasks_url(4), "/project/4?tab=tasks&q=is%3Aopen");
        assert_eq!(tasks_list_url("/org/3", None), "/org/3?tab=tasks");
        assert_eq!(
            tasks_list_url("/org/3", Some("is:done tag:x")),
            "/org/3?tab=tasks&q=is%3Adone%20tag%3Ax"
        );
        assert_eq!(quick_project_url(3), "/org/3/project/quick");
        assert_eq!(quick_board_project_url(2), "/board/2/project/quick");
        assert_eq!(
            folders_url("org:3", "my app", "/src", true),
            "/folders?home=org%3A3&name=my%20app&base_path=%2Fsrc&hidden=1"
        );
        assert_eq!(
            folders_url("board:2", "", "/", false),
            "/folders?home=board%3A2&base_path=%2F"
        );
    }

    #[test]
    fn the_nav_links_to_the_boards() {
        assert_eq!(super::super::ui::frame::NAV[0].1, BOARDS);
        assert_eq!(super::super::ui::frame::NAV[1].1, SETTINGS);
        assert_eq!(settings_saved_url(), "/settings?saved=1");
    }
}
