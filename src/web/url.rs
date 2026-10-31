//! The URLs of the pages, built in one place so no call site concatenates
//! a path by hand.

use super::task_query::DEFAULT;
use crate::reporting::fmt_date;
use chrono::NaiveDate;

pub fn page_url(base: &str, params: &[(&str, Option<&str>)]) -> String {
    let mut url = base.to_string();
    let mut sep = if base.contains('?') { '&' } else { '?' };
    for (key, value) in params {
        if let Some(value) = value {
            url.push(sep);
            url.push_str(key);
            url.push('=');
            url.push_str(&encode(value));
            sep = '&';
        }
    }
    url
}

fn encode(value: &str) -> String {
    let mut out = String::new();
    for b in value.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

pub const SETTINGS: &str = "/settings";

pub fn settings_saved_url() -> String {
    page_url(SETTINGS, &[("saved", Some("1"))])
}

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

pub fn edit_url(url: &str) -> String {
    format!("{url}/edit")
}

pub fn org_edit_url(id: i64) -> String {
    edit_url(&org_url(id))
}

pub fn task_edit_url(id: i64) -> String {
    edit_url(&task_url(id))
}

pub fn project_edit_url(id: i64) -> String {
    edit_url(&project_url(id))
}

pub fn board_url(id: i64) -> String {
    format!("/board/{id}")
}

pub fn board_day_url(id: i64, day: NaiveDate) -> String {
    page_url(&board_url(id), &[("date", Some(&fmt_date(day)))])
}

/// `side=off` when `folded`: every link of a board keeps it, so the column stays as it was left.
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

pub fn board_matrix_url(id: i64) -> String {
    format!("{}/matrix", board_url(id))
}

pub fn board_info_url(id: i64) -> String {
    format!("{}/info", board_url(id))
}

pub fn board_schedule_url(id: i64) -> String {
    format!("{}/schedule", board_url(id))
}

pub fn board_edit_url(id: i64) -> String {
    edit_url(&board_url(id))
}

/// Every link into the Tasks tab spells out the default search (`task_query::DEFAULT`), since a list with no `q` is unfiltered.
pub fn tasks_tab_url(base: &str) -> String {
    tasks_list_url(base, Some(DEFAULT))
}

/// `None` (what an emptied search box sends) is no filter at all.
pub fn tasks_list_url(base: &str, q: Option<&str>) -> String {
    page_url(base, &[("tab", Some("tasks")), ("q", q)])
}

fn tab(base: &str, name: &str) -> String {
    page_url(base, &[("tab", Some(name))])
}

pub fn notes_tab_url(base: &str) -> String {
    tab(base, "notes")
}

pub fn sessions_tab_url(base: &str) -> String {
    tab(base, "sessions")
}

pub fn projects_tab_url(base: &str) -> String {
    tab(base, "projects")
}

pub fn instructions_url() -> String {
    "/instructions".to_string()
}

pub fn instruction_url(name: &str, project: Option<i64>) -> String {
    let project = project.map(|p| p.to_string());
    page_url(
        &format!("/instruction/{}", encode(name)),
        &[("project", project.as_deref())],
    )
}

pub fn agent_tab_url(base: &str) -> String {
    tab(base, "agent")
}

pub fn wiki_tab_url(base: &str) -> String {
    tab(base, "wiki")
}

/// The URL builder and the route read the same words from here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WikiAction {
    Accept,
    Revert,
    Ingest,
}

impl WikiAction {
    pub fn as_str(self) -> &'static str {
        match self {
            WikiAction::Accept => "accept",
            WikiAction::Revert => "revert",
            WikiAction::Ingest => "ingest",
        }
    }
}

impl std::str::FromStr for WikiAction {
    type Err = String;

    fn from_str(word: &str) -> Result<Self, String> {
        [WikiAction::Accept, WikiAction::Revert, WikiAction::Ingest]
            .into_iter()
            .find(|a| a.as_str() == word)
            .ok_or_else(|| format!("'{word}' is not an action"))
    }
}

pub fn wiki_action_url(id: i64, action: WikiAction) -> String {
    format!("{}/wiki/{}", task_url(id), action.as_str())
}

pub fn task_step_url(id: i64, step: &str) -> String {
    page_url(
        &task_url(id),
        &[("tab", Some("agent")), ("step", Some(step))],
    )
}

pub fn plan_step_url(id: i64, step: &str) -> String {
    format!("{}/plan/{}", task_url(id), encode(step))
}

pub fn runs_url(id: i64) -> String {
    format!("{}/runs", task_url(id))
}

pub fn agents_url() -> String {
    "/agents".to_string()
}

pub fn agent_url(id: i64, n: u32) -> String {
    format!("/agent/{id}/{n}")
}

pub fn run_url(id: i64, n: u32) -> String {
    agent_url(id, n)
}

pub fn run_post_url(id: i64, n: u32) -> String {
    format!("{}/{n}", runs_url(id))
}

/// The URL and the routes (`task_runs.rs`) read the same words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunAction {
    Run,
    Cancel,
}

impl RunAction {
    pub fn as_str(self) -> &'static str {
        match self {
            RunAction::Run => "run",
            RunAction::Cancel => "cancel",
        }
    }
}

pub fn run_action_url(id: i64, n: u32, action: RunAction) -> String {
    format!("{}/{}", run_post_url(id, n), action.as_str())
}

pub fn note_url(base: &str, day: NaiveDate) -> String {
    format!("{base}/notes/{}", fmt_date(day))
}

pub fn note_tab_url(base: &str, day: NaiveDate, tab: super::sections::NoteTab) -> String {
    use super::sections::NoteTab;
    let name = (tab != NoteTab::Info).then(|| tab.as_str());
    page_url(&note_url(base, day), &[("tab", name)])
}

pub fn notes_today_url(base: &str) -> String {
    format!("{base}/notes/today")
}

pub const NEW_TASK: &str = "/task/new";

pub fn quick_board_project_url(id: i64) -> String {
    format!("{}/project/quick", board_url(id))
}

pub fn quick_project_url(id: i64) -> String {
    format!("{}/project/quick", org_url(id))
}

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
    fn step_urls_open_the_agent_tab_and_post_to_the_step() {
        assert_eq!(task_step_url(5, "s3"), "/task/5?tab=agent&step=s3");
        assert_eq!(plan_step_url(5, "s3"), "/task/5/plan/s3");
        assert_eq!(instruction_url("fix", None), "/instruction/fix");
        assert_eq!(
            instruction_url("fix", Some(3)),
            "/instruction/fix?project=3"
        );
        assert_eq!(projects_tab_url("/org/3"), "/org/3?tab=projects");
        assert_eq!(runs_url(5), "/task/5/runs");
        assert_eq!(run_url(5, 2), "/agent/5/2");
        assert_eq!(agents_url(), "/agents");
        assert_eq!(
            run_action_url(5, 2, RunAction::Cancel),
            "/task/5/runs/2/cancel"
        );
        assert_eq!(wiki_tab_url("/task/5"), "/task/5?tab=wiki");
        assert_eq!(
            wiki_action_url(5, WikiAction::Accept),
            "/task/5/wiki/accept"
        );
        for action in [WikiAction::Accept, WikiAction::Revert, WikiAction::Ingest] {
            assert_eq!(action.as_str().parse(), Ok(action));
        }
        assert!("explode".parse::<WikiAction>().is_err());
        // whatever the id, it is one path segment
        assert_eq!(plan_step_url(5, "a/b c"), "/task/5/plan/a%2Fb%20c");
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
    }

    #[test]
    fn the_nav_links_to_the_boards() {
        assert_eq!(super::super::ui::frame::NAV[0].1, BOARDS);
        assert_eq!(super::super::ui::frame::NAV[1].1, agents_url());
        assert_eq!(super::super::ui::frame::NAV[3].1, SETTINGS);
        assert_eq!(settings_saved_url(), "/settings?saved=1");
    }

    #[test]
    fn note_tab_urls_name_the_tab() {
        use super::super::sections::NoteTab;
        let d = NaiveDate::from_ymd_opt(2026, 10, 29).unwrap();
        assert_eq!(
            note_tab_url("/org/1", d, NoteTab::Info),
            "/org/1/notes/2026-10-29"
        );
        assert_eq!(
            note_tab_url("/org/1", d, NoteTab::Edit),
            "/org/1/notes/2026-10-29?tab=edit"
        );
    }
}
