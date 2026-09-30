//! The mapping from projects to what a project list shows
//! (`ui::project_list`): the one place a project's link is built.

use super::ui::empty_line::empty_line;
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
fn tilde(path: &str) -> String {
    match std::env::var("HOME") {
        Ok(dir) if !dir.is_empty() && path.starts_with(&dir) => {
            format!("~{}", &path[dir.len()..])
        }
        _ => path.to_string(),
    }
}

/// One row per project: the link and the base path shortened with `~`.
pub fn project_items(projects: &[Project]) -> Vec<ProjectItem<'_>> {
    projects
        .iter()
        .map(|p| ProjectItem {
            name: &p.name,
            href: project_url(p.id()),
            path: tilde(&p.base_path),
        })
        .collect()
}

/// The "Projects" section of an Info body: the count as a badge, then the
/// rows, or `empty` ("No projects.") when there are none. Shared by the
/// organization and board Info tabs.
#[component]
pub async fn projects_section(projects: &[Project], empty: &str) -> Result<impl View> {
    let items = project_items(projects);
    Ok(view! {
        labelled_section(id: "projects-title", label: "Projects", count: Some(projects.len()),
            if items.is_empty() {
                empty_line((empty))
            } else {
                project_list(items: &items)
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
}
