//! The sections of an organization or project page (`?tab=`) and their tab
//! rows: the parsing and the URLs live here, `ui/tabs.rs` only draws.

use super::ui::tabs::Tab;
use super::url::{
    agent_tab_url, notes_tab_url, projects_tab_url, sessions_tab_url, tasks_tab_url, wiki_tab_url,
};
use topcoat::router::query_params;

/// The query of the pages with tabs: `?tab=`, and the organization report's
/// period (`org_report::load`; the project page ignores it).
#[query_params(error = bad_request)]
pub struct TabQuery {
    pub tab: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    /// The Tasks tab's search (see `task_query.rs`), or the Info tab's
    /// project search.
    pub q: Option<String>,
    /// The pop-up open over the page: `task` (New task, on Tasks) or
    /// `project` (New project, on Info).
    pub new: Option<String>,
    /// The Report tab's Share pop-up is open (`?share=1`).
    pub share: Option<String>,
    /// What New project starts with, when the folder browser sends it back.
    pub name: Option<String>,
    pub base_path: Option<String>,
    /// The task page's Agent tab: the step open over the agents.
    pub step: Option<String>,
    /// The Agent tab: the agent open (`?run=3`), and the instruction a new
    /// one starts with (`?new=agent&instruction=fix`; `new` is shared with
    /// the other pages' pop-ups).
    pub run: Option<String>,
    /// An agent's Output tab: the text as it is (`?tab=output&raw=1`).
    pub raw: Option<String>,
    pub instruction: Option<String>,
    /// An instruction page: which tab is open is `tab`; `edit` opens its form.
    pub edit: Option<String>,
    /// An instruction page: the project whose instructions apply (none: the
    /// global ones).
    pub project: Option<String>,
}

/// The query of a page that is only a search (`?q=`): a board's Info.
#[query_params(error = bad_request)]
pub struct InfoQuery {
    pub q: Option<String>,
    /// As on [`TabQuery`]: `project` opens New project.
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
    /// An organization's: its projects.
    Projects,
    /// A project's: the instructions that apply to it (`instructions.rs`).
    Instructions,
}

impl Section {
    /// The section a `?tab=` value asks for: anything unknown is Info.
    pub fn of(tab: Option<&str>) -> Section {
        match tab {
            Some("tasks") => Section::Tasks,
            Some("notes") => Section::Notes,
            Some("report") => Section::Report,
            Some("projects") => Section::Projects,
            _ => Section::Info,
        }
    }

    /// The page title of the section: Info is the page itself (`page`, e.g.
    /// "Project"), the others say which one they are.
    pub fn title(self, page: &'static str) -> &'static str {
        match self {
            Section::Info => page,
            Section::Tasks => "Tasks",
            Section::Notes => "Notes",
            Section::Report => "Report",
            Section::Projects => "Projects",
            Section::Instructions => "Instructions",
        }
    }
}

/// The tabs of a page whose sections are the values of `S`: one per entry,
/// the one for `current` marked. Every page's tab row is built with this.
pub fn tabs_for<S: PartialEq>(current: S, entries: impl IntoIterator<Item = (S, Tab)>) -> Vec<Tab> {
    entries
        .into_iter()
        .map(|(section, tab)| tab.marked(section == current))
        .collect()
}

/// The last tab of a page that can be edited: Settings, which leads to the
/// edit form (`{base}/edit`). `current` is whether the form is showing.
pub fn settings_tab(base: &str, current: bool) -> Tab {
    Tab::link("Settings", format!("{base}/edit")).marked(current)
}

/// The Info, Projects (when `projects` says how many), Tasks and Notes tabs of
/// the page at `base` (the Tasks one with its count, the Notes one with `notes`, the number of notes when the page
/// has a folder for them), and the Report tab when the page has one:
/// `report` is its URL.
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

/// The tabs of a task page.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TaskSection {
    Info,
    Sessions,
    Agent,
    Notes,
    Wiki,
    /// The edit form (`/task/{id}/edit`), which shows the page's tab row.
    Settings,
}

impl TaskSection {
    /// The section a `?tab=` value asks for: anything unknown is Info.
    pub fn of(tab: Option<&str>) -> TaskSection {
        match tab {
            Some("sessions") => TaskSection::Sessions,
            Some("agent") => TaskSection::Agent,
            Some("notes") => TaskSection::Notes,
            Some("wiki") => TaskSection::Wiki,
            _ => TaskSection::Info,
        }
    }

    /// The page title of the section: Info is the page itself (`page`, "Task"),
    /// the others say which one they are.
    pub fn title(self, page: &'static str) -> &'static str {
        match self {
            TaskSection::Info => page,
            TaskSection::Sessions => "Sessions",
            TaskSection::Agent => "Agent",
            TaskSection::Notes => "Notes",
            TaskSection::Wiki => "Wiki",
            TaskSection::Settings => "Settings",
        }
    }
}

/// The Info, Sessions (with its count), Agent and Wiki tabs of the task page at
/// `base`.
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
            (TaskSection::Wiki, Tab::link("Wiki", wiki_tab_url(base))),
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
        assert_eq!(TaskSection::of(Some("wiki")), TaskSection::Wiki);
        assert_eq!(TaskSection::of(Some("tasks")), TaskSection::Info);
        assert_eq!(TaskSection::Info.title("Task"), "Task");
        assert_eq!(TaskSection::Agent.title("Task"), "Agent");
        assert_eq!(TaskSection::of(Some("settings")), TaskSection::Info);
    }

    #[test]
    fn task_tabs_mark_the_current_one() {
        let items = task_tabs("/task/7", TaskSection::Sessions, 3, 2);
        assert_eq!(items.len(), 6);
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
