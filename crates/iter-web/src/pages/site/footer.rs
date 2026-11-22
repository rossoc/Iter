//! The footer of every page (`ui/site_footer.rs`); one that cannot be built is left out, never breaking a page.

use crate::prelude::*;
use iter_core::{
    db::{Db, Table},
    models::{Board, Project},
};

/// Bounded: the footer is on every page, so what it reads must stay small however large the database.
pub(crate) const FOOTER_LIMIT: usize = 8;

fn link(name: &str, href: String) -> FooterLink {
    FooterLink {
        name: name.to_string(),
        href,
    }
}

fn project_links(projects: &[Project]) -> Vec<FooterLink> {
    projects
        .iter()
        .map(|p| link(&p.name, project_url(p.id())))
        .collect()
}

pub fn load(db: &Db) -> iter_core::error::Result<FooterData> {
    let orgs = db
        .organizations_with_projects(FOOTER_LIMIT)?
        .into_iter()
        .map(|(org, projects, total)| FooterGroup {
            link: link(&org.name, org_url(org.id())),
            projects: project_links(&projects),
            projects_total: total,
        })
        .collect();
    let (boards, boards_total) = db.first::<Board>(FOOTER_LIMIT)?;
    let (loose, loose_total) = db.loose_projects(FOOTER_LIMIT)?;
    Ok(FooterData {
        orgs,
        boards: boards
            .iter()
            .map(|b| link(&b.name, board_url(b.id())))
            .collect(),
        boards_total,
        boards_href: BOARDS,
        loose: project_links(&loose),
        loose_total,
        loose_href: "/",
        settings_href: SETTINGS.to_string(),
        version: env!("CARGO_PKG_VERSION"),
    })
}

#[component]
pub async fn site_footer_for() -> Result<impl View> {
    let data = fresh_db().ok().and_then(|db| load(&db).ok());
    Ok(view! {
        if let Some(data) = &data {
            site_footer(data: data)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use iter_core::{fixtures, models::Organization};

    fn org(db: &Db, name: &str) -> i64 {
        db.insert(&Organization {
            name: name.into(),
            ..Organization::default()
        })
        .expect("org")
    }

    fn add_projects(db: &Db, org: i64, names: impl IntoIterator<Item = String>) {
        for name in names {
            let mut p = fixtures::project(&name);
            p.organization_id = Some(org);
            db.insert(&p).expect("project");
        }
    }

    #[test]
    fn the_footer_lists_organizations_with_projects_boards_and_loose_projects() {
        let db = fixtures::db();
        let org = org(&db, "acme");
        add_projects(&db, org, ["web".into()]);
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
        let acme = org(&db, "acme");
        let empty = org(&db, "zeta");
        add_projects(
            &db,
            acme,
            (0..FOOTER_LIMIT + 2).map(|i| format!("web{i:02}")),
        );
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
        let org = org(&db, "acme");
        add_projects(&db, org, ["api".into(), "web".into()]);
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
