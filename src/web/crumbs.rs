//! The crumbs of the pages under an organization and/or project: which
//! entity is a crumb, and where it links.

use super::ui::breadcrumb::Crumb;
use super::url::{BOARDS, org_url, project_tasks_url};
use crate::db::Table;
use crate::models::{Organization, Project};

fn org_crumb(org: &Organization) -> Crumb {
    Crumb::link(org.name.as_str(), org_url(org.id()))
}

/// A page of a task sits under the project's Tasks tab; an edit page sits under the page it edits.
pub fn trail(org: &Option<Organization>, project: Option<&Project>, here: Crumb) -> Vec<Crumb> {
    let mut crumbs: Vec<Crumb> = org.iter().map(org_crumb).collect();
    if let Some(p) = project {
        crumbs.push(Crumb::link(p.name.as_str(), project_tasks_url(p.id())));
    }
    crumbs.push(here);
    crumbs
}

pub fn boards_crumb() -> Crumb {
    Crumb::link("Boards", BOARDS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(crumbs: &[Crumb]) -> Vec<(&str, Option<&str>, bool)> {
        crumbs
            .iter()
            .map(|c| (c.label.as_str(), c.href.as_deref(), c.current))
            .collect()
    }

    #[test]
    fn a_trail_runs_from_the_organization_to_the_page() {
        let project = Project {
            id: Some(4),
            name: "app".into(),
            ..Project::default()
        };
        assert_eq!(
            labels(&trail(&None, Some(&project), Crumb::here("New task"))),
            [
                ("app", Some("/project/4?tab=tasks&q=is%3Aopen"), false),
                ("New task", None, true)
            ]
        );
        let org = Some(Organization {
            id: Some(2),
            name: "acme".into(),
            ..Organization::default()
        });
        assert_eq!(
            labels(&trail(&org, None, Crumb::label("Project"))),
            [("acme", Some("/org/2"), false), ("Project", None, false)]
        );
    }
}
