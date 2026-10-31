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
            url.push_str(&encode(value));
            sep = '&';
        }
    }
    url
}

/// `value` percent-encoded: letters, digits and `-._~` stay, every other
/// byte becomes `%XX`. Fit for a query value or a path segment.
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

/// The Notes tab of the organization or project page at `base`.
pub fn notes_tab_url(base: &str) -> String {
    page_url(base, &[("tab", Some("notes"))])
}

/// The Sessions tab of the task page at `base`.
pub fn sessions_tab_url(base: &str) -> String {
    page_url(base, &[("tab", Some("sessions"))])
}

/// The Projects tab of the organization page at `base`.
pub fn projects_tab_url(base: &str) -> String {
    page_url(base, &[("tab", Some("projects"))])
}

/// The Instructions tab of the project page at `base`.
pub fn instructions_tab_url(base: &str) -> String {
    page_url(base, &[("tab", Some("instructions"))])
}

/// The list of the global instructions.
pub fn instructions_url() -> String {
    "/instructions".to_string()
}

/// The page of instruction `name`: the global one, or as it applies to
/// project `project`.
pub fn instruction_url(name: &str, project: Option<i64>) -> String {
    let project = project.map(|p| p.to_string());
    page_url(
        &format!("/instruction/{}", encode(name)),
        &[("project", project.as_deref())],
    )
}

/// The Agent tab of the task page at `base`.
pub fn agent_tab_url(base: &str) -> String {
    page_url(base, &[("tab", Some("agent"))])
}

/// The Wiki tab of the task page at `base`.
pub fn wiki_tab_url(base: &str) -> String {
    page_url(base, &[("tab", Some("wiki"))])
}

/// What a Wiki tab button does. The URL builder and the route read the same
/// words from here, so they cannot drift apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WikiAction {
    /// Keep the agent's changes.
    Accept,
    /// Throw them away.
    Revert,
    /// Start an ingest now.
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

/// Where the Wiki tab's buttons post.
pub fn wiki_action_url(id: i64, action: WikiAction) -> String {
    format!("{}/wiki/{}", task_url(id), action.as_str())
}

/// The Agent tab of task `id` with `step` open beside the plan.
pub fn task_step_url(id: i64, step: &str) -> String {
    page_url(
        &task_url(id),
        &[("tab", Some("agent")), ("step", Some(step))],
    )
}

/// Where a step's new state is posted.
pub fn plan_step_url(id: i64, step: &str) -> String {
    format!("{}/plan/{}", task_url(id), encode(step))
}

/// Where the form of a new agent for task `id` is posted.
pub fn runs_url(id: i64) -> String {
    format!("{}/runs", task_url(id))
}

/// The list of every agent of every task.
pub fn agents_url() -> String {
    "/agents".to_string()
}

/// The page of agent `n` of task `id`.
pub fn agent_url(id: i64, n: u32) -> String {
    format!("/agent/{id}/{n}")
}

/// Where a link to agent `n` of task `id` leads: its page.
pub fn run_url(id: i64, n: u32) -> String {
    agent_url(id, n)
}

/// Where the form of a queued agent is posted.
pub fn run_post_url(id: i64, n: u32) -> String {
    format!("{}/{n}", runs_url(id))
}

/// Where a button on agent `n` posts: `run` or `cancel`.
pub fn run_action_url(id: i64, n: u32, action: &str) -> String {
    format!("{}/{action}", run_post_url(id, n))
}

/// The note of `day` on the page at `base`.
pub fn note_url(base: &str, day: NaiveDate) -> String {
    format!("{base}/notes/{}", fmt_date(day))
}

/// Where "today's note" of the page at `base` is posted: it makes the note
/// and goes to it.
pub fn notes_today_url(base: &str) -> String {
    format!("{base}/notes/today")
}

/// Where the New task pop-up of a task list posts (`task_new.rs`).
pub const NEW_TASK: &str = "/task/new";

/// The folder browser the project create row opens.
pub const FOLDERS: &str = "/folders";

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
        assert_eq!(run_action_url(5, 2, "cancel"), "/task/5/runs/2/cancel");
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
        assert_eq!(super::super::ui::frame::NAV[1].1, agents_url());
        assert_eq!(super::super::ui::frame::NAV[3].1, SETTINGS);
        assert_eq!(settings_saved_url(), "/settings?saved=1");
    }
}
