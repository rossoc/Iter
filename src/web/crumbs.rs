//! The crumbs of the pages under an organization and/or project: which
//! entity is a crumb, and where it links.

use super::ui::breadcrumb::Crumb;
use super::url::{BOARDS, org_url, project_tasks_url};
use crate::db::Table;
use crate::models::{Organization, Project};

/// The organization, linked to its page: the first crumb of every page under
/// one ([`trail`], [`edit_trail`]).
fn org_crumb(org: &Organization) -> Crumb {
    Crumb::link(org.name.as_str(), org_url(org.id()))
}

/// Where a crumb for a project or organization leads, one rule: the page
/// the ancestor is the parent of. A page of a task (the task pages, New task)
/// sits under the project's Tasks tab, so [`trail`] links the project there;
/// an edit page sits under the page it edits, so [`edit_trail`] links the
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

/// The crumbs of an edit page: [`trail`] down to the entity, which is
/// `name` linked to `page_url` (the page being edited), then "Edit".
/// An organization's edit page passes no organization and no project; a
/// project's passes its organization; a task's both.
pub fn edit_trail(
    org: &Option<Organization>,
    project: Option<&Project>,
    name: &str,
    page_url: &str,
) -> Vec<Crumb> {
    edit_tail(trail(org, project, Crumb::link(name, page_url)))
}

/// The first crumb of every board page: the list of boards.
pub fn boards_crumb() -> Crumb {
    Crumb::link("Boards", BOARDS)
}

/// The crumbs of a board's edit page: `Boards / {name} / Edit`, `name`
/// linked to `page_url` (the page being edited): [`edit_trail`] for a board.
pub fn board_edit_trail(name: &str, page_url: &str) -> Vec<Crumb> {
    edit_tail(vec![boards_crumb(), Crumb::link(name, page_url)])
}

/// `crumbs` (down to the page being edited) and "Edit": the one ending every
/// edit trail has.
fn edit_tail(mut crumbs: Vec<Crumb>) -> Vec<Crumb> {
    crumbs.push(Crumb::here("Edit"));
    crumbs
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
                ("app", Some("/project/4?tab=tasks"), false),
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
        assert_eq!(
            labels(&edit_trail(&org, None, "app", "/project/4")),
            [
                ("acme", Some("/org/2"), false),
                ("app", Some("/project/4"), false),
                ("Edit", None, true)
            ]
        );
        assert_eq!(
            labels(&edit_trail(&org, Some(&project), "fix", "/task/9")),
            [
                ("acme", Some("/org/2"), false),
                ("app", Some("/project/4?tab=tasks"), false),
                ("fix", Some("/task/9"), false),
                ("Edit", None, true)
            ]
        );
        assert_eq!(
            labels(&board_edit_trail("work", "/board/3/info")),
            [
                ("Boards", Some("/boards"), false),
                ("work", Some("/board/3/info"), false),
                ("Edit", None, true)
            ]
        );
        assert_eq!(
            labels(&edit_trail(&None, None, "acme", "/org/2")),
            [("acme", Some("/org/2"), false), ("Edit", None, true)]
        );
    }
}
