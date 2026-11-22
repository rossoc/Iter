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
    Settings,
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
            Section::Settings => "Settings",
        }
    }
}

pub fn tabs_for<S: PartialEq>(current: S, entries: impl IntoIterator<Item = (S, Tab)>) -> Vec<Tab> {
    entries
        .into_iter()
        .map(|(section, tab)| tab.marked(section == current))
        .collect()
}

/// What decides which tabs an organization or project page has, and their badges.
#[derive(Default)]
pub struct Counts {
    pub projects: Option<usize>,
    pub tasks: usize,
    pub notes: Option<usize>,
    /// The Report tab's link (organizations only).
    pub report: Option<String>,
    pub wiki: bool,
}

/// Info, Projects, Tasks, Notes, Report, Wiki, Settings: those `counts` names.
pub fn section_tabs(base: &str, current: Section, counts: Counts) -> Vec<Tab> {
    let Counts {
        projects,
        tasks,
        notes,
        report,
        wiki,
    } = counts;
    let entries = [
        Some((Section::Info, Tab::link("Info", base.to_string()))),
        projects.map(|n| {
            (
                Section::Projects,
                Tab::link("Projects", projects_tab_url(base)).count(n),
            )
        }),
        Some((
            Section::Tasks,
            Tab::link("Tasks", tasks_tab_url(base)).count(tasks),
        )),
        Some((
            Section::Notes,
            Tab::link("Notes", notes_tab_url(base)).count(notes),
        )),
        report.map(|href| (Section::Report, Tab::link("Report", href))),
        wiki.then(|| (Section::Wiki, Tab::link("Wiki", wiki_tab_url(base)))),
        Some((Section::Settings, Tab::link("Settings", edit_url(base)))),
    ];
    tabs_for(current, entries.into_iter().flatten())
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
            (TaskSection::Settings, Tab::link("Settings", edit_url(base))),
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
    fn the_settings_row_keeps_the_badges_and_ends_with_settings() {
        let items = task_tabs("/task/7", TaskSection::Settings, 3, 2);
        assert_eq!(items.len(), 5);
        assert!(items[..4].iter().all(|tab| !tab.is_current()));
        assert!(items[4].is_current() && items[4].href() == "/task/7/edit");
        assert_eq!((items[1].badge(), items[2].badge()), (Some(3), Some(2)));
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
        let counts = |projects, notes, report| Counts {
            projects,
            tasks: 4,
            notes,
            report,
            wiki: false,
        };
        let items = section_tabs("/project/1", Section::Tasks, counts(None, None, None));
        assert_eq!(items.len(), 4);
        assert!(!items[0].is_current() && items[1].is_current());
        assert_eq!(
            (items[1].href(), items[1].badge()),
            ("/project/1?tab=tasks&q=is%3Aopen", Some(4))
        );
        let org = section_tabs(
            "/org/1",
            Section::Settings,
            counts(Some(9), Some(2), Some("/r".into())),
        );
        assert_eq!(org.len(), 6);
        assert!(org[5].is_current() && org[5].href() == "/org/1/edit");
    }
}
