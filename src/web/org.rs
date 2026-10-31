//! The organization page (`/org/{id}`): the route, the loading and the
//! view of the Info and Tasks tabs. The Report tab is `org_report.rs`. The
//! page's own rules are in `org.css` (the report tab only, bundled into
//! `/style.css`); the shared ones are the components in `ui/`.

use super::Id;
use super::assets::Asset;
use super::daily_notes::{NotesTab, notes_section};
use super::load::tasks_or_count;
use super::org_report::{Report, report_link, report_view, tab_url};
use super::org_share::{Outcome, ShareDialog, preview_modal, share_modal};
use super::project_new::{Home, ProjectCreate, project_modal};
use super::project_rows::{project_search_in, projects_section};
use super::sections::{Section, TabQuery, section_tabs, settings_tab};
use super::settings_panel::settings_panel;
use super::task_new::{TaskCreate, TaskHome, task_modal};
use super::task_query::TaskQuery;
use super::task_rows::{group_rows, search};
use super::text::NO_PROJECTS;
use super::ui::frame::frame;
use super::ui::labelled::mono_value;
use super::ui::layout::info_columns;
use super::ui::page_header::{Kicker, page_header};
use super::ui::prose::description;
use super::ui::tabs::tabs;
use super::ui::tasks_tab::tasks_tab;
use super::url::org_url;
use super::{now, open_db};
use crate::db::{Db, Table};
use crate::models::{Configured, Organization, Project, Task};
use crate::reporting::fmt_date;
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
    builder.route(script).page(show)
}

static ORG_JS: Asset = Asset::js(include_str!("org.js"));

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
    /// The notes' folder; the notes themselves only for the Notes tab.
    pub notes: NotesTab,
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
    let notes = NotesTab::load(crate::notes::org_dir(&org), section == Section::Notes);
    Ok(Loaded {
        org,
        projects,
        tasks,
        task_count,
        notes,
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
        (Section::Projects, Some("project")) => {
            Some(Home::Org(id).create(query.name.as_deref(), query.base_path.as_deref())?)
        }
        _ => None,
    };
    // `?share=1`: the Share pop-up over the Report tab
    let share_dialog = match (section, query.share.as_deref(), &page.report) {
        (Section::Report, Some(_), Some(report)) if !report.projects.is_empty() => {
            let base = org_url(id);
            let close = report_link(
                &base,
                Some(&report.range.from.map(fmt_date).unwrap_or_default()),
                Some(&fmt_date(report.range.to)),
            );
            Some(ShareDialog::new(&page.org, &base, report, close))
        }
        _ => None,
    };
    Ok(view! {
        screen(page: &page, section: section, today: now.date(), q: q, task_create: task_create.as_ref(), project_create: project_create.as_ref(), share_dialog: share_dialog.as_ref())
    })
}

/// The Info tab: description and notes path, the Settings aside.
#[component]
async fn info(org: &Organization) -> Result<impl View> {
    Ok(view! {
        info_columns(
            <div>
                description(text: &org.description)
                if !org.notes_path.is_empty() {
                    mono_value(id: "notes-path-title", label: "Notes path", value: &org.notes_path)
                }
            </div>
            settings_panel(s: org.settings())
        )
    })
}

/// The Projects tab: the search box, the list, its first row opening New project.
#[component]
async fn projects_tab(
    org: &Organization,
    projects: &[Project],
    q: Option<&str>,
    add: String,
) -> Result<impl View> {
    let search = project_search_in(org_url(org.id()), "projects", q);
    Ok(view! {
        projects_section(projects: projects, empty: NO_PROJECTS, search: Some(search), add: Some(add), titled: false)
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
    #[default] share: Option<&Outcome>,
    #[default] share_dialog: Option<&ShareDialog>,
) -> Result<impl View> {
    let Loaded {
        org,
        projects,
        tasks,
        task_count,
        notes,
        report,
    } = page;
    let base = org_url(org.id());
    let mut items = section_tabs(
        &base,
        section,
        Some(projects.len()),
        *task_count,
        NotesTab::badge(Some(notes)),
        Some(tab_url(&base, today)),
    );
    items.push(settings_tab(&base, false));
    let parts = [section.title("Organization"), org.name.as_str()];
    let rows = group_rows(tasks);
    // the first row of Tasks opens New task, when there is a project to put it in
    let add_task = (!projects.is_empty()).then(|| TaskHome::Org(org.id()).open_url(q));
    let add_project = Home::Org(org.id()).open_url();
    // a push preview is a pop-up too; closing it goes back to the report
    let preview = match (share, report) {
        (Some(Outcome::Preview(p)), Some(r)) => {
            let close = report_link(
                &base,
                Some(&r.range.from.map(fmt_date).unwrap_or_default()),
                Some(&fmt_date(r.range.to)),
            );
            Some((p.as_ref(), base.clone(), close))
        }
        _ => None,
    };
    let dialog = match (task_create, project_create, share_dialog, preview) {
        (Some(create), ..) => Some(Child::new(view! { task_modal(create: create) })),
        (None, Some(create), ..) => Some(Child::new(view! { project_modal(create: create) })),
        (None, None, Some(dialog), _) => Some(Child::new(view! { share_modal(dialog: dialog) })),
        (None, None, None, Some((p, base, close))) => Some(Child::new(
            view! { preview_modal(base: base, p: p, close: close) },
        )),
        (None, None, None, None) => None,
    };
    let refused = task_create.is_some_and(|c| c.error.is_some())
        || project_create.is_some_and(|c| c.error.is_some());
    Ok(view! {
        frame(
            title: &parts,
            error: refused,
            dialog: dialog,
            page_header(title: &org.name, kicker: Kicker::Eyebrow("Organization"))
            tabs(label: "Sections", items: &items)
            if section == Section::Tasks {
                tasks_tab(rows: &rows, caption: "Tasks in this organization", total: *task_count, search: search(&base, q), add: add_task)
            } else if section == Section::Notes {
                notes_section(base: &base, notes: notes)
            } else if section == Section::Projects {
                projects_tab(org: org, projects: projects, q: q, add: add_project)
            } else if let Some(r) = report {
                report_view(base: &base, report: r, today: today, shared: share)
            } else {
                info(org: org)
            }
        )
    })
}
