//! The projects of a group (a board, an organization) as the edit forms'
//! checklists list them, one builder and one checklist for both: "the
//! projects belonging to X", each ticked when it does, and a project that
//! belongs to another group says so, since ticking it moves it here.

use super::forms::PROJECTS;
use super::ui::field::{CheckOption, checklist};
use super::ui::form_error::FormError;
use super::ui::group_card::NO_PROJECTS;
use crate::db::{Db, Table};
use crate::models::ProjectGroup;
use std::collections::{HashMap, HashSet};
use topcoat::{
    Result,
    view::{View, component, view},
};

/// The Projects checklist of a group's edit form. `noun` names the group
/// kind in the hint ("organization", "board").
#[component]
pub async fn project_checklist(
    options: &[CheckOption],
    noun: &str,
    #[default] error: Option<&FormError>,
) -> Result<impl View> {
    let hint = format!("Ticking a project moves it here. Unticking one moves it off this {noun}.");
    Ok(view! {
        checklist(
            name: PROJECTS,
            label: "Projects",
            options: options,
            hint: &hint,
            empty: NO_PROJECTS,
            error: error
        )
    })
}

/// [`project_options`] over every group of kind `G`, read here.
pub fn group_options<G: ProjectGroup + Table>(
    db: &Db,
    id: i64,
    ticked: Option<&[i64]>,
) -> crate::error::Result<Vec<CheckOption>> {
    project_options(db, &db.list::<G>()?, id, ticked)
}

/// Every project as an option, in name order. `groups` are the groups of
/// the kind (`id` may be among them: it is not "elsewhere"). `ticked` are
/// the ids ticked in a submission; none means the projects in group `id`.
/// The other groups' names come from one map and the ticks from one set:
/// one pass, not a lookup per project per option.
pub fn project_options<G: ProjectGroup + Table>(
    db: &Db,
    groups: &[G],
    id: i64,
    ticked: Option<&[i64]>,
) -> crate::error::Result<Vec<CheckOption>> {
    Ok(build::<G>(db.project_owners::<G>()?, groups, id, ticked))
}

fn build<G: ProjectGroup + Table>(
    projects: Vec<(i64, String, Option<i64>)>,
    groups: &[G],
    id: i64,
    ticked: Option<&[i64]>,
) -> Vec<CheckOption> {
    let names: HashMap<i64, &str> = groups.iter().map(|g| (g.id(), g.name())).collect();
    let ticked: Option<HashSet<i64>> = ticked.map(|ids| ids.iter().copied().collect());
    projects
        .into_iter()
        .map(|(project, name, owner)| {
            let on = match &ticked {
                Some(ids) => ids.contains(&project),
                None => owner == Some(id),
            };
            let note = owner
                .filter(|&other| other != id)
                .and_then(|other| names.get(&other))
                .map(|group| format!("(currently in {group})"))
                .unwrap_or_default();
            CheckOption {
                value: project.to_string(),
                label: name,
                note,
                on,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Board, Organization};

    fn board(id: i64, name: &str) -> Board {
        Board {
            id: Some(id),
            name: name.into(),
            description: String::new(),
        }
    }

    fn projects() -> Vec<(i64, String, Option<i64>)> {
        vec![
            (10, "a".into(), Some(1)),
            (11, "b".into(), Some(2)),
            (12, "c".into(), None),
        ]
    }

    fn seen(options: &[CheckOption]) -> Vec<(&str, &str, bool)> {
        options
            .iter()
            .map(|o| (o.label.as_str(), o.note.as_str(), o.on))
            .collect()
    }

    #[test]
    fn the_groups_projects_are_ticked_and_the_others_are_named() {
        let boards = [board(1, "work"), board(2, "home")];
        let got = build::<Board>(projects(), &boards, 1, None);
        assert_eq!(
            seen(&got),
            [
                ("a", "", true),
                ("b", "(currently in home)", false),
                ("c", "", false)
            ]
        );
        assert_eq!(got[1].value, "11");
    }

    /// After a refusal the ticks are the submission's, wherever the
    /// projects are stored.
    #[test]
    fn a_submission_decides_the_ticks_after_a_refusal() {
        let boards = [board(1, "work"), board(2, "home")];
        let got = build::<Board>(projects(), &boards, 1, Some(&[11]));
        assert_eq!(
            seen(&got),
            [
                ("a", "", false),
                ("b", "(currently in home)", true),
                ("c", "", false)
            ]
        );
    }

    #[test]
    fn organizations_are_listed_the_same_way() {
        let org = |id, name: &str| Organization {
            id: Some(id),
            name: name.into(),
            ..Organization::default()
        };
        let orgs = [org(1, "acme"), org(2, "beta")];
        let got = build::<Organization>(projects(), &orgs, 2, None);
        assert_eq!(
            seen(&got),
            [
                ("a", "(currently in acme)", false),
                ("b", "", true),
                ("c", "", false)
            ]
        );
    }
}
