//! The crumbs of the pages under an organization and/or project: which
//! entity is a crumb, and where it links.

use super::ui::breadcrumb::Crumb;
use super::url::{BOARDS, org_url, project_tasks_url};
use crate::db::Table;
use crate::models::{Organization, Project};

/// The organization, linked to its page: the first crumb of every page under
/// one ([`trail`]).
fn org_crumb(org: &Organization) -> Crumb {
    Crumb::link(org.name.as_str(), org_url(org.id()))
}

/// Where a crumb for a project or organization leads, one rule: the page
/// the ancestor is the parent of. A page of a task (the task pages, New task)
/// sits under the project's Tasks tab, so [`trail`] links the project there;
/// an edit page sits under the page it edits, so the edit trail links the
/// entity to that page (its Info; Cancel goes there too).
///
/// The crumbs of a page under an organization and/or project: the
/// organization (if any), the project (linked to its Tasks tab), then
/// `here`, the page itself. Task pages pass the project and their own crumb.
pub fn trail(org: &Option<Organization>, project: Option<&Project>, here: Crumb) -> Vec<Crumb> {
    let mut crumbs: Vec<Crumb> = org.iter().map(org_crumb).collect();
    if let Some(p) = project {
        crumbs.push(Crumb::link(p.name.as_str(), project_tasks_url(p.id())));
    }
    crumbs.push(here);
    crumbs
}

/// The first crumb of every board page: the list of boards.
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
