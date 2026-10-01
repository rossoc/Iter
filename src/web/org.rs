//! The organization page (`/org/{id}`): the route, the loading and the
//! view of the Info and Tasks tabs. The Report tab is `org_report.rs`. The
//! page's own rules are in `/org.css` (the report tab only); the shared ones
//! are the components in `ui/`.

use super::Id;
use super::assets::Asset;
use super::load::tasks_or_count;
use super::org_report::{Report, report_view, tab_url};
use super::project_new::{Home, ProjectCreate, project_modal};
use super::project_rows::{project_search, projects_section};
use super::sections::{Section, TabQuery, section_tabs};
use super::settings_panel::settings_panel;
use super::task_new::{TaskCreate, TaskHome, task_modal};
use super::task_query::TaskQuery;
use super::task_rows::{group_rows, search};
use super::ui::button::edit_button;
use super::ui::columns::info_columns;
use super::ui::frame::frame;
use super::ui::group_card::NO_PROJECTS;
use super::ui::page_header::{Kicker, page_header};
use super::ui::prose::description;
use super::ui::tabs::tabs;
use super::ui::tasks_tab::tasks_tab;
use super::url::{edit_url, org_url};
use super::{now, open_db};
use crate::db::{Db, Table};
use crate::models::{Configured, Organization, Project, Task};
use chrono::{NaiveDate, NaiveDateTime};
use topcoat::{
    Result,
    context::Cx,
    router::{
        RouterBuilder, error::RouterErrorExt, page, path_param, query_params, response::Response,
        route,
    },
    view::{Child, View, component, view},
};

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder.route(stylesheet).route(script).page(show)
}

static ORG_CSS: Asset = Asset::css(include_str!("org.css"));
static ORG_JS: Asset = Asset::js(include_str!("org.js"));

#[route(GET "/org.css")]
async fn stylesheet(cx: &Cx) -> Result<Response> {
    ORG_CSS.respond(cx)
}

#[route(GET "/org.js")]
async fn script(cx: &Cx) -> Result<Response> {
    ORG_JS.respond(cx)
}

/// What the organization page shows.
pub struct Loaded {
    pub org: Organization,
    pub projects: Vec<Project>,
    /// Only for the Tasks tab.
    pub tasks: Vec<(String, Task)>,
    /// The tab badge: one `COUNT` on the other tabs, the rows counted on
    /// Tasks.
    pub task_count: usize,
    /// Only for the Report tab.
    pub report: Option<Report>,
}

/// Loads organization `id` for `section`: the rows of the Tasks tab or the
/// report only for their own tab. `from`/`to` are the report's period.
pub fn load(
    db: &Db,
    id: i64,
    section: Section,
    from: Option<&str>,
    to: Option<&str>,
    q: Option<&str>,
    now: NaiveDateTime,
) -> Result<Loaded> {
    let org = db.get::<Organization>(id)?.ok_or_not_found()?;
    let projects = db.projects_in::<Organization>(id)?;
    let (tasks, task_count) = tasks_or_count(
        section == Section::Tasks,
        || db.tasks_in::<Organization>(id),
        || db.count_tasks_in::<Organization>(id),
    )?;
    // the badge keeps the total; the rows are the ones the search lets by
    let tasks = TaskQuery::of(q).filter_named(db, tasks)?;
    let report = match section {
        Section::Report => Some(super::org_report::load(db, &projects, from, to, now)?),
        _ => None,
    };
    Ok(Loaded {
        org,
        projects,
        tasks,
        task_count,
        report,
    })
}

#[page("/org/{id}")]
async fn show(cx: &Cx) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let query = query_params::<TabQuery>(cx)?;
    let section = Section::of(query.tab.as_deref());
    let now = now();
    let page = load(
        &open_db()?,
        id,
        section,
        query.from.as_deref(),
        query.to.as_deref(),
        query.q.as_deref(),
        now,
    )?;
    let q = query.q.as_deref();
    // `?new=`: the New task pop-up over Tasks, New project over Info
    let task_create = match (section, query.new.as_deref()) {
        (Section::Tasks, Some("task")) => TaskCreate::in_org(id, &page.projects, q),
        _ => None,
    };
    let project_create = match (section, query.new.as_deref()) {
        (Section::Info, Some("project")) => {
            Some(Home::Org(id).create(query.name.as_deref(), query.base_path.as_deref())?)
        }
        _ => None,
    };
    Ok(view! {
        screen(page: &page, section: section, today: now.date(), q: q, task_create: task_create.as_ref(), project_create: project_create.as_ref())
    })
}

#[component]
async fn info(
    org: &Organization,
    projects: &[Project],
    q: Option<&str>,
    add: String,
) -> Result<impl View> {
    let search = project_search(org_url(org.id()), q);
    Ok(view! {
        info_columns(
            <div>
                description(text: &org.description)
                projects_section(projects: projects, empty: NO_PROJECTS, search: Some(search), add: Some(add))
            </div>
            settings_panel(s: org.settings())
        )
    })
}

/// `q` is the page's search (the Tasks tab's or the project list's, by
/// `section`). `task_create` and `project_create` are the New task and New
/// project pop-ups, when one is open (also when it comes back refused, with
/// what was typed and why).
#[component]
pub async fn screen(
    page: &Loaded,
    section: Section,
    today: NaiveDate,
    #[default] q: Option<&str>,
    #[default] task_create: Option<&TaskCreate>,
    #[default] project_create: Option<&ProjectCreate>,
) -> Result<impl View> {
    let Loaded {
        org,
        projects,
        tasks,
        task_count,
        report,
    } = page;
    let base = org_url(org.id());
    let items = section_tabs(&base, section, *task_count, Some(tab_url(&base, today)));
    let parts = [section.title("Organization"), org.name.as_str()];
    let rows = group_rows(tasks);
    // the first row of Tasks opens New task, when there is a project to put it in
    let add_task = (!projects.is_empty()).then(|| TaskHome::Org(org.id()).open_url(q));
    let add_project = Home::Org(org.id()).open_url();
    let dialog = match (task_create, project_create) {
        (Some(create), _) => Some(Child::new(view! { task_modal(create: create) })),
        (None, Some(create)) => Some(Child::new(view! { project_modal(create: create) })),
        (None, None) => None,
    };
    Ok(view! {
        frame(
            title: &parts,
            styles: &["/org.css"],
            dialog: dialog,
            page_header(title: &org.name, kicker: Kicker::Eyebrow("Organization"), edit_button(href: edit_url(&base)))
            tabs(label: "Sections", items: &items)
            if section == Section::Tasks {
                tasks_tab(rows: &rows, caption: "Tasks in this organization", total: *task_count, search: search(&base, q), add: add_task)
            } else if let Some(r) = report {
                report_view(base: &base, report: r, today: today)
            } else {
                info(org: org, projects: projects, q: q, add: add_project)
            }
        )
    })
}
