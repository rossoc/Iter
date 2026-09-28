//! The page frame: the top navbar (Home | Board), the sidebar
//! (organizations, with their projects below) and the main pane a page
//! fills.

use crate::db::{Db, Table};
use crate::models::{Organization, Project};
use topcoat::{
    Result,
    router::{Slot, content::{Css, Js}, layout, route},
    view::{Child, View, component, view},
};

/// What the sidebar highlights.
#[derive(Clone, Copy, PartialEq)]
pub enum Sel {
    None,
    Org(i64),
    Project(i64),
    /// Any board page. Boards are their own section, so the sidebar of
    /// organizations and projects steps aside for them.
    Board,
}

/// The sidebar's contents, loaded up front so rendering needs no database.
pub struct Nav {
    pub orgs: Vec<(Organization, Vec<Project>)>,
    /// Projects that belong to no organization.
    pub loose: Vec<Project>,
}

impl Nav {
    pub fn load(db: &Db) -> crate::error::Result<Nav> {
        let mut orgs = Vec::new();
        for org in db.list::<Organization>()? {
            let projects = db.projects_for_organization(org.id())?;
            orgs.push((org, projects));
        }
        let loose = db
            .list::<Project>()?
            .into_iter()
            .filter(|p| p.organization_id.is_none())
            .collect();
        Ok(Nav { orgs, loose })
    }
}

fn cls(on: bool) -> &'static str {
    if on { "sel" } else { "" }
}

/// The frame around a page's content.
#[component]
pub async fn shell(nav: &Nav, sel: Sel, child: Child<'_>) -> Result<impl View> {
    Ok(view! {
        <header class="top">
            <a class=(cls(sel != Sel::Board)) href="/">"Home"</a>
            <a class=(cls(sel == Sel::Board)) href="/boards">"Board"</a>
        </header>
        <div class="app">
            if sel != Sel::Board {
            <nav class="side">
                <h2>"Organizations"</h2>
                <ul>
                    for (org, projects) in nav.orgs.iter() {
                        <li>
                            <a class=("org ".to_string() + cls(sel == Sel::Org(org.id())))
                               href=(format!("/org/{}", org.id()))>(org.name.as_str())</a>
                            <ul>
                                for p in projects.iter() {
                                    <li>
                                        <a class=(cls(sel == Sel::Project(p.id())))
                                           href=(format!("/project/{}", p.id()))>(p.name.as_str())</a>
                                    </li>
                                }
                            </ul>
                        </li>
                    }
                </ul>
                <h2>"Projects"</h2>
                <ul>
                    for p in nav.loose.iter() {
                        <li>
                            <a class=(cls(sel == Sel::Project(p.id())))
                               href=(format!("/project/{}", p.id()))>(p.name.as_str())</a>
                        </li>
                    }
                </ul>
            </nav>
            }
            <main>(child)</main>
        </div>
    })
}

#[layout("/")]
pub async fn root(slot: Slot<'_>) -> Result<impl View> {
    Ok(view! {
        <!DOCTYPE html>
        <html>
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <title>"iter"</title>
                <link rel="stylesheet" href="/style.css">
            </head>
            <body>(slot)</body>
        </html>
    })
}

#[route(GET "/board.js")]
pub async fn board_script() -> Result<Js<&'static str>> {
    Ok(Js(include_str!("board.js")))
}

#[route(GET "/style.css")]
pub async fn stylesheet() -> Result<Css<&'static str>> {
    Ok(Css(include_str!("style.css")))
}
