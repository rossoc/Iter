//! The footer of every page (`ui/site_footer.rs`): loads what it shows and
//! renders it after the page, from the layout. A footer that cannot be built
//! (no database, a read error) is left out: it must never break a page.

use super::load::{Grouped, grouped};
use super::open_db;
use super::ui::site_footer::{FooterData, FooterGroup, FooterLink, site_footer};
use super::url::{SETTINGS, board_url, org_url, project_url};
use crate::db::{Db, Table};
use crate::models::{Board, Organization, Project};
use topcoat::{
    Result,
    view::{View, component, view},
};

fn project_link(p: &Project) -> FooterLink {
    FooterLink {
        name: p.name.clone(),
        href: project_url(p.id()),
    }
}

/// What the footer shows: the organizations with their projects (two
/// queries), and the boards (one).
pub fn load(db: &Db) -> crate::error::Result<FooterData> {
    let Grouped { groups, loose } = grouped::<Organization>(db)?;
    let orgs = groups
        .iter()
        .map(|(org, projects)| FooterGroup {
            link: FooterLink {
                name: org.name.clone(),
                href: org_url(org.id()),
            },
            projects: projects.iter().map(project_link).collect(),
        })
        .collect();
    let boards = db
        .list::<Board>()?
        .iter()
        .map(|b| FooterLink {
            name: b.name.clone(),
            href: board_url(b.id()),
        })
        .collect();
    Ok(FooterData {
        orgs,
        boards,
        loose: loose.iter().map(project_link).collect(),
        settings_href: SETTINGS.to_string(),
        version: env!("CARGO_PKG_VERSION"),
    })
}

/// The footer of a page, or nothing when it cannot be loaded.
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
}
