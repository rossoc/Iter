//! The home page (`/`): a grid of the organizations with their projects. It
//! has no rules of its own.

use super::load::{Grouped, grouped};
use super::notes::first_line;
use super::open_db;
use super::project_rows::project_items;
use super::ui::FOLDER;
use super::ui::empty_state::empty_state;
use super::ui::frame::frame;
use super::ui::group_card::{NO_PROJECTS, card_grid, group_card};
use super::ui::page_header::{Kicker, lede, page_header};
use super::url::org_url;
use crate::db::Table;
use crate::models::{Organization, Project};
use topcoat::{
    Result,
    router::{RouterBuilder, page},
    view::{View, component, view},
};

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder.page(home)
}

#[page("/")]
async fn home() -> Result<impl View> {
    let Grouped { groups, loose } = grouped::<Organization>(&open_db()?)?;
    Ok(view! { overview(orgs: &groups, loose: &loose) })
}

/// One card of the overview grid.
#[component]
async fn org_card(org: Option<&Organization>, projects: &[Project]) -> Result<impl View> {
    let items = project_items(projects);
    Ok(view! {
        group_card(
            title: org.map_or("No organization", |o| o.name.as_str()),
            href: org.map(|o| org_url(o.id())),
            note: org.map_or("", |o| first_line(&o.description)),
            items: &items,
            none: NO_PROJECTS
        )
    })
}

#[component]
async fn overview(orgs: &[(Organization, Vec<Project>)], loose: &[Project]) -> Result<impl View> {
    let empty = orgs.is_empty() && loose.is_empty();
    Ok(view! {
        frame(
            title: &[],
            page_header(title: "Where to next?", kicker: Kicker::Eyebrow("Home"))
            lede("Select an organization or a project.")
            if empty {
                empty_state(
                    svg: FOLDER,
                    "No projects yet. Create one with "<code>"iter init"</code>" or "<code>"iter new <path>"</code>"."
                )
            } else {
                card_grid(
                    for (org, projects) in orgs.iter() {
                        org_card(org: Some(org), projects: projects)
                    }
                    if !loose.is_empty() {
                        org_card(org: None, projects: loose)
                    }
                )
            }
        )
    })
}
