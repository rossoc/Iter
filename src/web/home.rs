//! The home page, in two designs side by side while the new one is being
//! reviewed: `/` is the proposal, `/?design=old` the page as it was (see
//! `v2.rs`). The proposal's own rules are in `/home.css`.

use super::layout::{Nav, Sel, shell};
use super::open_db;
use super::v2::{ARROW, FOLDER, compare, frame, icon, is_old, tilde};
use crate::db::Table;
use crate::models::{Organization, Project};
use topcoat::{
    Result,
    context::Cx,
    router::{RouterBuilder, content::Css, page, query_params, route},
    view::{View, component, view},
};

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder.page(home).route(home_stylesheet)
}

#[route(GET "/home.css")]
async fn home_stylesheet() -> Result<Css<&'static str>> {
    Ok(Css(include_str!("home.css")))
}

#[query_params(error = bad_request)]
struct DesignQuery {
    design: Option<String>,
}

#[page("/")]
async fn home(cx: &Cx) -> Result<impl View> {
    let old = is_old(&query_params::<DesignQuery>(cx)?.design);
    let nav = Nav::load(&open_db()?)?;
    Ok(view! {
        if old {
            shell(
                nav: &nav,
                sel: Sel::None,
                <p class="empty">"Select an organization or a project."</p>
                compare(old: true, here: "/")
            )
        } else {
            proposal(nav: &nav)
            compare(old: false, here: "/")
        }
    })
}

// ---- the proposal -------------------------------------------------------------

/// The first line of an organization's markdown notes, as a one-line summary.
fn summary(text: &str) -> String {
    text.lines()
        .map(|l| l.trim().trim_start_matches('#').trim())
        .find(|l| !l.is_empty())
        .unwrap_or_default()
        .to_string()
}

/// One card of the overview grid.
#[component]
async fn group_card(org: Option<&Organization>, projects: &[Project]) -> Result<impl View> {
    let (title, href, note) = match org {
        Some(o) => (
            o.name.clone(),
            format!("/org/{}", o.id()),
            summary(&o.description),
        ),
        None => ("No organization".to_string(), String::new(), String::new()),
    };
    Ok(view! {
        <article class="group">
            <header>
                if href.is_empty() {
                    <h2>(title.clone())</h2>
                } else {
                    <h2><a href=(href.clone())><span class="u">(title.clone())</span> (icon(ARROW))</a></h2>
                }
            </header>
            if !note.is_empty() {
                <p class="note">(note.clone())</p>
            }
            if projects.is_empty() {
                <p class="none">"No projects yet."</p>
            } else {
                <ul>
                    for p in projects.iter() {
                        <li>
                            <a class="u" href=(format!("/project/{}", p.id()))>(p.name.clone())</a>
                            <span class="path"><bdi>(tilde(&p.base_path))</bdi></span>
                        </li>
                    }
                </ul>
            }
        </article>
    })
}

#[component]
async fn proposal(nav: &Nav) -> Result<impl View> {
    let empty = nav.orgs.is_empty() && nav.loose.is_empty();
    Ok(view! {
        frame(
            css: Some("/home.css"),
            <p class="label">"Home"</p>
            <h1>"Where to next?"</h1>
            <p class="lede">"Select an organization or a project."</p>
            if empty {
                <div class="empty">
                    (icon(FOLDER))
                    <p>"Nothing here yet. Create a project with "<code>"iter init"</code>" or "<code>"iter new <path>"</code>"."</p>
                </div>
            } else {
                <section class="grid">
                    for (org, projects) in nav.orgs.iter() {
                        group_card(org: Some(org), projects: projects)
                    }
                    if !nav.loose.is_empty() {
                        group_card(org: None, projects: &nav.loose)
                    }
                </section>
            }
        )
    })
}
