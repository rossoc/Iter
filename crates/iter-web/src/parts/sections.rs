//! The sections of an organization or project page (`?tab=`) and their tab
//! rows: the parsing and the URLs live here, `ui/tabs.rs` only draws.

use crate::prelude::*;

/// The query of the pages with tabs: `?tab=`, and the organization report's
/// period (`org_report::load`; the project page ignores it).
#[query_params(error = bad_request)]
pub struct TabQuery {
    pub tab: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub q: Option<String>,
    /// The pop-up open over the page: `task` (New task, on Tasks) or
    /// `project` (New project, on Info).
    pub new: Option<String>,
    pub share: Option<String>,
    /// The Report after a push: the hash of the worklog pushed (its ledger row is shown).
    pub pushed: Option<String>,
    pub name: Option<String>,
    pub base_path: Option<String>,
    pub step: Option<String>,
    /// The Agent tab: the agent open (`?run=3`), and the instruction a new
    /// one starts with (`?new=agent&instruction=fix`; `new` is shared with
    /// the other pages' pop-ups).
    pub run: Option<String>,
    pub raw: Option<String>,
    pub instruction: Option<String>,
    pub edit: Option<String>,
    pub project: Option<String>,
}

#[query_params(error = bad_request)]
pub struct InfoQuery {
    pub q: Option<String>,
    pub new: Option<String>,
    pub name: Option<String>,
    pub base_path: Option<String>,
}

/// The tabs of an organization or project page. A project has no Report.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Section {
    Info,
    Tasks,
    Notes,
    Report,
    Projects,
    /// A project's wiki (the organization page has none).
    Wiki,
}

impl Section {
    pub fn of(tab: Option<&str>) -> Section {
        match tab {
            Some("tasks") => Section::Tasks,
            Some("notes") => Section::Notes,
            Some("report") => Section::Report,
            Some("projects") => Section::Projects,
            Some("wiki") => Section::Wiki,
            _ => Section::Info,
        }
    }

    /// Info is the page itself (`page`); the others say which one they are.
    pub fn title(self, page: &'static str) -> &'static str {
        match self {
            Section::Info => page,
            Section::Tasks => "Tasks",
            Section::Notes => "Notes",
            Section::Report => "Report",
            Section::Projects => "Projects",
            Section::Wiki => "Wiki",
        }
    }
}

pub fn tabs_for<S: PartialEq>(current: S, entries: impl IntoIterator<Item = (S, Tab)>) -> Vec<Tab> {
    entries
        .into_iter()
        .map(|(section, tab)| tab.marked(section == current))
        .collect()
}

pub fn settings_tab(base: &str, current: bool) -> Tab {
    Tab::link("Settings", format!("{base}/edit")).marked(current)
}

pub fn section_tabs(
    base: &str,
    current: Section,
    projects: Option<usize>,
    task_count: usize,
    notes: Option<usize>,
    report: Option<String>,
) -> Vec<Tab> {
    let mut entries = vec![(Section::Info, Tab::link("Info", base.to_string()))];
    if let Some(n) = projects {
        entries.push((
            Section::Projects,
            Tab::link("Projects", projects_tab_url(base)).count(n),
        ));
    }
    entries.extend([
        (
            Section::Tasks,
            Tab::link("Tasks", tasks_tab_url(base)).count(task_count),
        ),
        (Section::Notes, {
            let tab = Tab::link("Notes", notes_tab_url(base));
            match notes {
                Some(n) => tab.count(n),
                None => tab,
            }
        }),
    ]);
    if let Some(href) = report {
        entries.push((Section::Report, Tab::link("Report", href)));
    }
    tabs_for(current, entries)
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TaskSection {
    Info,
    Sessions,
    Agent,
    Notes,
    Settings,
}

impl TaskSection {
    pub fn of(tab: Option<&str>) -> TaskSection {
        match tab {
            Some("sessions") => TaskSection::Sessions,
            Some("agent") => TaskSection::Agent,
            Some("notes") => TaskSection::Notes,
            _ => TaskSection::Info,
        }
    }

    /// Info is the page itself (`page`); the others say which one they are.
    pub fn title(self, page: &'static str) -> &'static str {
        match self {
            TaskSection::Info => page,
            TaskSection::Sessions => "Sessions",
            TaskSection::Agent => "Agent",
            TaskSection::Notes => "Notes",
            TaskSection::Settings => "Settings",
        }
    }
}

pub fn task_tabs(base: &str, current: TaskSection, sessions: usize, agents: usize) -> Vec<Tab> {
    tabs_for(
        current,
        [
            (TaskSection::Info, Tab::link("Info", base.to_string())),
            (
                TaskSection::Sessions,
                Tab::link("Sessions", sessions_tab_url(base)).count(sessions),
            ),
            (
                TaskSection::Agent,
                Tab::link("Agents", agent_tab_url(base)).count(agents),
            ),
            (TaskSection::Notes, Tab::link("Notes", notes_tab_url(base))),
            (
                TaskSection::Settings,
                Tab::link("Settings", format!("{base}/edit")),
            ),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_query_value_picks_a_task_section() {
        assert_eq!(TaskSection::of(None), TaskSection::Info);
        assert_eq!(TaskSection::of(Some("sessions")), TaskSection::Sessions);
        assert_eq!(TaskSection::of(Some("agent")), TaskSection::Agent);
        assert_eq!(Section::of(Some("wiki")), Section::Wiki);
        assert_eq!(TaskSection::of(Some("tasks")), TaskSection::Info);
        assert_eq!(TaskSection::Info.title("Task"), "Task");
        assert_eq!(TaskSection::Agent.title("Task"), "Agent");
        assert_eq!(TaskSection::of(Some("settings")), TaskSection::Info);
    }

    #[test]
    fn task_tabs_mark_the_current_one() {
        let items = task_tabs("/task/7", TaskSection::Sessions, 3, 2);
        // Info, Sessions, Agents, Notes, Settings: the wiki is the project's
        assert_eq!(items.len(), 5);
        assert!(!items[0].is_current() && items[1].is_current() && !items[2].is_current());
        assert_eq!(
            (items[1].href(), items[1].badge()),
            ("/task/7?tab=sessions", Some(3))
        );
        assert_eq!(items[2].href(), "/task/7?tab=agent");
    }

    #[test]
    fn a_query_value_picks_a_section() {
        assert_eq!(Section::of(None), Section::Info);
        assert_eq!(Section::of(Some("tasks")), Section::Tasks);
        assert_eq!(Section::of(Some("report")), Section::Report);
        assert_eq!(Section::of(Some("notes")), Section::Notes);
        assert_eq!(Section::of(Some("x")), Section::Info);
        assert_eq!(Section::Info.title("Project"), "Project");
        assert_eq!(Section::Tasks.title("Project"), "Tasks");
    }

    #[test]
    fn tabs_for_marks_only_the_current_entry() {
        let items = tabs_for(
            2,
            [
                (1, Tab::link("a", "/a".into())),
                (2, Tab::link("b", "/b".into())),
            ],
        );
        assert!(!items[0].is_current() && items[1].is_current());
    }

    #[test]
    fn section_tabs_mark_the_current_one() {
        let items = section_tabs("/project/1", Section::Tasks, None, 4, None, None);
        assert_eq!(items.len(), 3);
        assert!(!items[0].is_current() && items[1].is_current());
        assert_eq!(
            (items[1].href(), items[1].badge()),
            ("/project/1?tab=tasks&q=is%3Aopen", Some(4))
        );
        assert_eq!(
            section_tabs(
                "/org/1",
                Section::Info,
                Some(9),
                0,
                Some(2),
                Some("/r".into())
            )
            .len(),
            5
        );
    }
}
