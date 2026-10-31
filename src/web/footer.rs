//! The footer of every page (`ui/site_footer.rs`); one that cannot be built is left out, never breaking a page.

use super::open_db;
use super::ui::site_footer::{FooterData, FooterGroup, FooterLink, site_footer};
use super::url::{BOARDS, SETTINGS, board_url, org_url, project_url};
use crate::db::{Db, Table};
use crate::models::{Board, Project};
use topcoat::{
    Result,
    view::{View, component, view},
};

/// Bounded: the footer is on every page, so what it reads must stay small however large the database.
pub const FOOTER_LIMIT: usize = 8;

fn project_link(p: &Project) -> FooterLink {
    FooterLink {
        name: p.name.clone(),
        href: project_url(p.id()),
    }
}

pub fn load(db: &Db) -> crate::error::Result<FooterData> {
    let orgs = db
        .organizations_with_projects(FOOTER_LIMIT)?
        .into_iter()
        .map(|(org, projects, total)| FooterGroup {
            link: FooterLink {
                name: org.name.clone(),
                href: org_url(org.id()),
            },
            projects: projects.iter().map(project_link).collect(),
            projects_total: total,
        })
        .collect();
    let (boards, boards_total) = db.first_by_name::<Board>("", FOOTER_LIMIT)?;
    let (loose, loose_total) = db.loose_projects(FOOTER_LIMIT)?;
    Ok(FooterData {
        orgs,
        boards: boards
            .iter()
            .map(|b| FooterLink {
                name: b.name.clone(),
                href: board_url(b.id()),
            })
            .collect(),
        boards_total,
        boards_href: BOARDS,
        loose: loose.iter().map(project_link).collect(),
        loose_total,
        loose_href: "/",
        settings_href: SETTINGS.to_string(),
        version: env!("CARGO_PKG_VERSION"),
    })
}

#[component]
pub async fn site_footer_for() -> Result<impl View> {
    let data = open_db().ok().and_then(|db| load(&db).ok());
    Ok(view! {
        if let Some(data) = &data {
            site_footer(data: data)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::models::Organization;

    #[test]
    fn the_footer_lists_organizations_with_projects_boards_and_loose_projects() {
        let db = fixtures::db();
        let org = db
            .insert(&Organization {
                name: "acme".into(),
                ..Organization::default()
            })
            .expect("org");
        let mut in_org = fixtures::project("web");
        in_org.organization_id = Some(org);
        db.insert(&in_org).expect("project");
        fixtures::add_project(&db, "solo");
        let board = db.insert(&fixtures::board("week")).expect("board");

        let data = load(&db).expect("footer data");
        assert_eq!(data.orgs.len(), 1);
        assert_eq!(data.orgs[0].link.href, format!("/org/{org}"));
        assert_eq!(data.orgs[0].projects.len(), 1);
        assert_eq!(data.orgs[0].projects[0].name, "web");
        assert_eq!(
            data.loose
                .iter()
                .map(|l| l.name.as_str())
                .collect::<Vec<_>>(),
            ["solo"]
        );
        assert_eq!(data.boards[0].href, format!("/board/{board}"));
        assert_eq!(data.settings_href, "/settings");
        assert_eq!(data.version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn a_list_past_the_limit_is_cut_and_counts_what_it_leaves_out() {
        let db = fixtures::db();
        let acme = db
            .insert(&Organization {
                name: "acme".into(),
                ..Organization::default()
            })
            .expect("org");
        let empty = db
            .insert(&Organization {
                name: "zeta".into(),
                ..Organization::default()
            })
            .expect("org");
        for i in 0..FOOTER_LIMIT + 2 {
            let mut p = fixtures::project(&format!("web{i:02}"));
            p.organization_id = Some(acme);
            db.insert(&p).expect("project");
        }
        for i in 0..FOOTER_LIMIT + 1 {
            fixtures::add_project(&db, &format!("solo{i:02}"));
            db.insert(&fixtures::board(&format!("week{i:02}")))
                .expect("board");
        }

        let data = load(&db).expect("footer data");
        assert_eq!(data.orgs.len(), 2);
        assert_eq!(data.orgs[0].link.name, "acme");
        assert_eq!(data.orgs[0].projects.len(), FOOTER_LIMIT);
        assert_eq!(data.orgs[0].projects[0].name, "web00");
        assert_eq!(data.orgs[0].projects_total, FOOTER_LIMIT + 2);
        assert_eq!(data.orgs[1].link.href, format!("/org/{empty}"));
        assert!(data.orgs[1].projects.is_empty());
        assert_eq!(data.orgs[1].projects_total, 0);
        assert_eq!(data.loose.len(), FOOTER_LIMIT);
        assert_eq!(data.loose[0].name, "solo00");
        assert_eq!(data.loose_total, FOOTER_LIMIT + 1);
        assert_eq!(data.boards.len(), FOOTER_LIMIT);
        assert_eq!(data.boards_total, FOOTER_LIMIT + 1);
    }

    #[test]
    fn a_list_within_the_limit_is_not_cut() {
        let db = fixtures::db();
        let org = db
            .insert(&Organization {
                name: "acme".into(),
                ..Organization::default()
            })
            .expect("org");
        for name in ["api", "web"] {
            let mut p = fixtures::project(name);
            p.organization_id = Some(org);
            db.insert(&p).expect("project");
        }
        fixtures::add_project(&db, "solo");
        db.insert(&fixtures::board("week")).expect("board");

        let data = load(&db).expect("footer data");
        assert_eq!(data.orgs[0].projects.len(), 2);
        assert_eq!(data.orgs[0].projects_total, 2);
        assert_eq!(data.loose.len(), 1);
        assert_eq!(data.loose_total, 1);
        assert_eq!(data.boards.len(), 1);
        assert_eq!(data.boards_total, 1);
    }
}
