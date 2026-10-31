//! Lookups shared by several pages' `load` functions.

use crate::db::{Db, Table};
use crate::models::{Board, Card, Organization, Project, ProjectGroup, Task};
use std::collections::HashMap;
use topcoat::router::error::RouterErrorExt;

pub fn org_of(db: &Db, project: &Project) -> crate::error::Result<Option<Organization>> {
    Ok(match Organization::group_of(project) {
        Some(id) => db.get(id)?,
        None => None,
    })
}

pub fn org_in<'a>(orgs: &'a [Organization], project: &Project) -> Option<&'a Organization> {
    let id = project.organization_id?;
    orgs.iter().find(|o| o.id() == id)
}

pub struct Grouped<G> {
    pub groups: Vec<(G, Vec<Project>)>,
    pub loose: Vec<Project>,
}

pub fn grouped<G: Table + ProjectGroup>(db: &Db) -> crate::error::Result<Grouped<G>> {
    let mut members: HashMap<Option<i64>, Vec<Project>> = HashMap::new();
    for project in db.list::<Project>()? {
        members
            .entry(G::group_of(&project))
            .or_default()
            .push(project);
    }
    let groups = db
        .list::<G>()?
        .into_iter()
        .map(|group| {
            let mine = members.remove(&Some(group.id())).unwrap_or_default();
            (group, mine)
        })
        .collect();
    Ok(Grouped {
        groups,
        loose: members.remove(&None).unwrap_or_default(),
    })
}

pub fn board_of(db: &Db, id: i64) -> topcoat::Result<Board> {
    Ok(db.get::<Board>(id)?.ok_or_not_found()?)
}

/// Unfinished cards, urgent first, then by label; finished tasks are left out in the SQL.
pub fn unfinished(db: &Db, board_id: i64) -> crate::error::Result<Vec<Card>> {
    let mut cards = db.cards_unfinished(board_id)?;
    cards.sort_by(|a, b| (b.priority.urgent, &a.label).cmp(&(a.priority.urgent, &b.label)));
    Ok(cards)
}

/// Rows only when `wanted`; on other tabs just a `COUNT` for the badge.
pub fn tasks_or_count<T>(
    wanted: bool,
    rows: impl FnOnce() -> crate::error::Result<Vec<T>>,
    count: impl FnOnce() -> crate::error::Result<usize>,
) -> crate::error::Result<(Vec<T>, usize)> {
    if wanted {
        let rows = rows()?;
        let n = rows.len();
        Ok((rows, n))
    } else {
        Ok((Vec::new(), count()?))
    }
}

pub struct Parents {
    pub project: Project,
    pub org: Option<Organization>,
}

pub fn project_of(db: &Db, id: i64) -> topcoat::Result<Project> {
    Ok(db.get::<Project>(id)?.ok_or_not_found()?)
}

pub fn task_with_project(db: &Db, id: i64) -> topcoat::Result<(Task, Project)> {
    let task = db.get::<Task>(id)?.ok_or_not_found()?;
    let project = project_of(db, task.project_id)?;
    Ok((task, project))
}

pub fn parents_of(db: &Db, id: i64) -> topcoat::Result<Parents> {
    let project = project_of(db, id)?;
    let org = org_of(db, &project)?;
    Ok(Parents { project, org })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;

    #[test]
    fn parents_are_the_project_and_its_organization_only_when_it_has_one() {
        let db = fixtures::db();
        let org = Organization {
            name: "acme".into(),
            ..Organization::default()
        };
        let org_id = db.insert(&org).expect("org");
        let mut p = fixtures::project("in");
        p.organization_id = Some(org_id);
        let inside = db.insert(&p).expect("project");
        let outside = fixtures::add_project(&db, "out");
        let with = parents_of(&db, inside).expect("found");
        assert_eq!(with.org.map(|o| o.name), Some("acme".to_string()));
        let without = parents_of(&db, outside).expect("found");
        assert!(without.org.is_none());
        assert!(parents_of(&db, 999).is_err());
    }

    #[test]
    fn projects_are_grouped_under_their_own_group_and_the_rest_are_loose() {
        let db = fixtures::db();
        let work = db.insert(&fixtures::board("work")).expect("board");
        db.insert(&fixtures::board("home")).expect("board");
        for (name, board) in [("a", Some(work)), ("b", None)] {
            let mut p = fixtures::project(name);
            p.board_id = board;
            db.insert(&p).expect("project");
        }
        let boards = grouped::<Board>(&db).expect("load");
        let counts: Vec<(&str, usize)> = boards
            .groups
            .iter()
            .map(|(b, ps)| (b.name.as_str(), ps.len()))
            .collect();
        assert!(counts.contains(&("work", 1)) && counts.contains(&("home", 0)));
        assert_eq!(boards.loose.len(), 1);
        // every project is in no organization
        let orgs = grouped::<Organization>(&db).expect("load");
        assert!(orgs.groups.is_empty() && orgs.loose.len() == 2);
    }

    #[test]
    fn unfinished_cards_come_urgent_first_then_by_label() {
        use crate::models::{Priority, Task, TaskStatus};
        let db = fixtures::db();
        let board = db.insert(&fixtures::board("b")).expect("board");
        let mut p = fixtures::project("p");
        p.board_id = Some(board);
        let project = db.insert(&p).expect("project");
        let mut ids = Vec::new();
        for (name, status) in [
            ("b", TaskStatus::Queue),
            ("a", TaskStatus::Queue),
            ("z", TaskStatus::Queue),
            ("gone", TaskStatus::Done),
        ] {
            let mut task = Task::template(project, String::new(), status);
            task.name = name.into();
            ids.push(db.insert(&task).expect("task"));
        }
        db.set_priority(
            ids[2],
            Priority {
                urgent: true,
                important: false,
            },
        )
        .expect("flag");
        let labels: Vec<String> = unfinished(&db, board)
            .expect("cards")
            .into_iter()
            .map(|c| c.label)
            .collect();
        assert_eq!(labels, ["p/z", "p/a", "p/b"]);
    }

    #[test]
    fn a_missing_board_is_an_error() {
        let db = fixtures::db();
        assert!(board_of(&db, 1).is_err());
    }
}
