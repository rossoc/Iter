//! The page frame: the top navbar (Home | Board), the sidebar
//! (organizations, with their projects below) and the main pane a page
//! fills.

use crate::db::{Db, Table};
use crate::models::{Organization, Project};
use topcoat::{
    Result,
    router::{
        Slot,
        content::{Css, Js},
        layout, route,
    },
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
        // Two queries, grouped here, rather than one per organization.
        let projects = db.list::<Project>()?;
        let members = |id: Option<i64>| -> Vec<Project> {
            projects
                .iter()
                .filter(|p| p.organization_id == id)
                .cloned()
                .collect()
        };
        let orgs = db
            .list::<Organization>()?
            .into_iter()
            .map(|org| {
                let mine = members(org.id);
                (org, mine)
            })
            .collect();
        Ok(Nav {
            orgs,
            loose: members(None),
        })
    }
}

// ---- building blocks shared by every page --------------------------------

pub(super) fn cls(on: bool) -> &'static str {
    if on { "sel" } else { "" }
}

#[component]
pub(super) async fn description(text: &str) -> Result<impl View> {
    Ok(view! {
        if !text.is_empty() {
            <pre class="desc">(text.to_string())</pre>
        }
    })
}

#[component]
pub(super) async fn error_box(error: &Option<String>) -> Result<impl View> {
    Ok(view! {
        if let Some(message) = error {
            <div class="error">(message.clone())</div>
        }
    })
}

#[component]
pub(super) async fn check(name: &str, label: &str, on: bool) -> Result<impl View> {
    Ok(view! {
        <label class="check">
            <input type="checkbox" name=(name.to_string()) if on { checked="" }>
            (label.to_string())
        </label>
    })
}

#[component]
pub(super) async fn field(name: &str, label: &str, value: &str) -> Result<impl View> {
    Ok(view! {
        <label>(label.to_string())</label>
        <input type="text" name=(name.to_string()) value=(value.to_string())>
    })
}

#[component]
pub(super) async fn textarea(text: &str) -> Result<impl View> {
    Ok(view! {
        <label>"Description (markdown)"</label>
        <textarea name="description">(text.to_string())</textarea>
    })
}

#[component]
pub(super) async fn form_actions(#[into] cancel: String) -> Result<impl View> {
    Ok(view! {
        <button type="submit">"Save"</button>
        " "
        <a href=(cancel.clone())>"Cancel"</a>
    })
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
