//! The mapping from projects to what a project list shows
//! (`ui::project_list`): the one place a project's link is built.

use super::ui::empty_line::empty_line;
use super::ui::filter_bar::{Search, filter_bar};
use super::ui::labelled::labelled_section;
use super::ui::project_list::{ProjectItem, project_list};
use super::url::project_url;
use crate::db::Table;
use crate::models::Project;
use topcoat::{
    Result,
    view::{View, component, view},
};

/// `path` with the home directory shortened to `~`.
pub fn tilde(path: &str) -> String {
    match std::env::var("HOME") {
        Ok(dir) if !dir.is_empty() && path.starts_with(&dir) => {
            format!("~{}", &path[dir.len()..])
        }
        _ => path.to_string(),
    }
}

/// One row per project: the link and the base path shortened with `~`.
pub fn project_items(projects: &[Project]) -> Vec<ProjectItem<'_>> {
    items_of(projects.iter())
}

fn items_of<'a>(projects: impl Iterator<Item = &'a Project>) -> Vec<ProjectItem<'a>> {
    projects
        .map(|p| ProjectItem {
            name: &p.name,
            href: project_url(p.id()),
            path: tilde(&p.base_path),
        })
        .collect()
}

/// The projects whose name or base path contains every word of `q`, in any
/// case (blank: all).
pub fn matching<'a>(projects: &'a [Project], q: &str) -> Vec<&'a Project> {
    let words: Vec<String> = q.split_whitespace().map(str::to_lowercase).collect();
    projects
        .iter()
        .filter(|p| {
            let text = format!("{} {}", p.name, p.base_path).to_lowercase();
            words.iter().all(|w| text.contains(w))
        })
        .collect()
}

/// The syntax of the project search box, for its "?" (see [`matching`]).
const SYNTAX: &[(&str, &str)] = &[
    ("app", "in the project's name or base path, in any case"),
    ("web app", "every word must be there, in any order"),
    ("src/", "part of a path works too"),
];

/// The search box over a project list at `base` (the Info tab), showing `q`.
pub fn project_search(base: String, q: Option<&str>) -> Search {
    let q = q.unwrap_or("").trim();
    Search {
        clear: (!q.is_empty()).then(|| base.clone()),
        action: base,
        tab: "",
        q: q.to_string(),
        label: "Filter projects",
        syntax: SYNTAX,
    }
}

/// The "Projects" section of an Info body: the count as a badge, the search
/// box (when there are projects), then the rows -- the first one opens New
/// project when there is `add` (where it leads) -- or `empty` ("No
/// projects.") when there are none. Shared by the organization and board
/// Info tabs.
#[component]
pub async fn projects_section(
    projects: &[Project],
    empty: &str,
    #[default] search: Option<Search>,
    #[default] add: Option<String>,
) -> Result<impl View> {
    let shown = match &search {
        Some(s) => matching(projects, &s.q),
        None => projects.iter().collect(),
    };
    let items = items_of(shown.iter().copied());
    Ok(view! {
        labelled_section(id: "projects-title", label: "Projects", count: Some(projects.len()),
            if let Some(search) = &search {
                if !projects.is_empty() {
                    filter_bar(search: search)
                    if items.len() != projects.len() {
                        <p class="filter-count" role="status">(items.len()) " of " (projects.len()) " projects match"</p>
                    }
                }
            }
            if add.is_some() || !items.is_empty() {
                project_list(items: &items, add: add)
            }
            if projects.is_empty() {
                empty_line((empty))
            } else if items.is_empty() {
                empty_line("No projects match.")
            }
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_link_the_project_and_carry_its_path() {
        let projects = [Project {
            id: Some(4),
            name: "app".into(),
            base_path: "/src/app".into(),
            ..Project::default()
        }];
        let items = project_items(&projects);
        assert_eq!(
            (
                items[0].name,
                items[0].href.as_str(),
                items[0].path.as_str()
            ),
            ("app", "/project/4", "/src/app")
        );
    }

    #[test]
    fn a_search_matches_name_or_path_and_every_word() {
        let project = |name: &str, path: &str| Project {
            name: name.into(),
            base_path: path.into(),
            ..Project::default()
        };
        let all = [project("App", "/src/web"), project("tools", "/src/app-cli")];
        let names =
            |q: &str| -> Vec<&str> { matching(&all, q).iter().map(|p| p.name.as_str()).collect() };
        assert_eq!(names(""), ["App", "tools"]);
        assert_eq!(names("app"), ["App", "tools"]);
        assert_eq!(names("APP web"), ["App"]);
        assert_eq!(names("cli"), ["tools"]);
        assert!(names("zzz").is_empty());
    }
}
