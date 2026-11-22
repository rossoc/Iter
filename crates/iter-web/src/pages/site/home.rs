//! The home page (`/`): a grid of the organizations with their projects.

use crate::load::Grouped;
use crate::load::grouped;
use crate::parts::all::*;
use crate::prelude::*;
use iter_core::db::Table;
use iter_core::models::{Organization, Project};

#[page("/")]
pub(super) async fn home() -> Result<impl View> {
    let Grouped { groups, loose } = grouped::<Organization>(&fresh_db()?)?;
    Ok(view! { overview(orgs: &groups, loose: &loose) })
}

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
