//! The sections of an organization or project page (`?tab=`) and their tab
//! rows: the parsing and the URLs live here, `ui/tabs.rs` only draws.

use super::ui::tabs::Tab;
use super::url::tasks_tab_url;
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
    /// What New project starts with, when the folder browser sends it back.
    pub name: Option<String>,
    pub base_path: Option<String>,
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
    Report,
}

impl Section {
    /// The section a `?tab=` value asks for: anything unknown is Info.
    pub fn of(tab: Option<&str>) -> Section {
        match tab {
            Some("tasks") => Section::Tasks,
            Some("report") => Section::Report,
            _ => Section::Info,
        }
    }

    /// The page title of the section: Info is the page itself (`page`, e.g.
    /// "Project"), the others say which one they are.
    pub fn title(self, page: &'static str) -> &'static str {
        match self {
            Section::Info => page,
            Section::Tasks => "Tasks",
            Section::Report => "Report",
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

/// The Info and Tasks tabs of the page at `base` (the Tasks one with its
/// count), and the Report tab when the page has one: `report` is its URL.
pub fn section_tabs(
    base: &str,
    current: Section,
    task_count: usize,
    report: Option<String>,
) -> Vec<Tab> {
    let mut entries = vec![
        (Section::Info, Tab::link("Info", base.to_string())),
        (
            Section::Tasks,
            Tab::link("Tasks", tasks_tab_url(base)).count(task_count),
        ),
    ];
    if let Some(href) = report {
        entries.push((Section::Report, Tab::link("Report", href)));
    }
    tabs_for(current, entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_query_value_picks_a_section() {
        assert_eq!(Section::of(None), Section::Info);
        assert_eq!(Section::of(Some("tasks")), Section::Tasks);
        assert_eq!(Section::of(Some("report")), Section::Report);
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
        let items = section_tabs("/project/1", Section::Tasks, 4, None);
        assert_eq!(items.len(), 2);
        assert!(!items[0].is_current() && items[1].is_current());
        assert_eq!(
            (items[1].href(), items[1].badge()),
            ("/project/1?tab=tasks&q=is%3Aopen", Some(4))
        );
        assert_eq!(
            section_tabs("/org/1", Section::Info, 0, Some("/r".into())).len(),
            3
        );
    }
}
