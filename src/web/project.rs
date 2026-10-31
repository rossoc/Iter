//! The project page (`/project/{id}`): route, loading and view, built from shared components.

use super::Id;
use super::crumbs::trail;
use super::daily_notes::{NotesTab, notes_section};
use super::load::{Parents, parents_of, tasks_or_count};
use super::open_db;
use super::sections::{Section, TabQuery};
use super::sections::{section_tabs, settings_tab};
use super::settings_panel::settings_panel;
use super::task_new::{TaskCreate, TaskHome, task_modal};
use super::task_query::TaskQuery;
use super::task_rows::{project_rows, search};
use super::ui::breadcrumb::Crumb;
use super::ui::frame::frame;
use super::ui::labelled::mono_value;
use super::ui::layout::info_columns;
use super::ui::page_header::{Kicker, page_header};
use super::ui::prose::description;
use super::ui::tabs::tabs;
use super::ui::tasks_tab::tasks_tab;
use super::url::project_url;
use crate::db::{Db, Table};
use crate::models::{Configured, Organization, Project, Task};
use topcoat::{
    Result,
    context::Cx,
    router::{RouterBuilder, page, path_param, query_params},
    view::{Child, View, component, view},
};

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder.page(show)
}

/// A project has no Report tab: that value falls to Info.
pub fn section(tab: Option<&str>) -> Section {
    match Section::of(tab) {
        Section::Report => Section::Info,
        other => other,
    }
}

pub struct Loaded {
    pub parents: Parents,
    pub tasks: Vec<Task>,
    pub task_count: usize,
    pub notes: NotesTab,
}

pub fn load(db: &Db, id: i64, section: Section, q: Option<&str>) -> Result<Loaded> {
    let parents = parents_of(db, id)?;
    let (tasks, task_count) = tasks_or_count(
        section == Section::Tasks,
        || db.tasks_for_project(id),
        || db.count_tasks_for_project(id),
    )?;
    // the badge keeps the total; the rows are the ones the search lets by
    let tasks = TaskQuery::of(q).filter_tasks(db, tasks, &parents.project.name)?;
    let notes = NotesTab::load(
        crate::notes::project_dir(&parents.project, &parents.org),
        section == Section::Notes,
    );
    Ok(Loaded {
        parents,
        tasks,
        task_count,
        notes,
    })
}

#[page("/project/{id}")]
async fn show(cx: &Cx) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let query = query_params::<TabQuery>(cx)?;
    let tab = section(query.tab.as_deref());
    let page = load(&open_db()?, id, tab, query.q.as_deref())?;
    let Loaded {
        parents: Parents { project, org },
        tasks,
        task_count,
        notes,
    } = page;
    let q = query.q.as_deref();
    let create = (tab == Section::Tasks && query.new.as_deref() == Some("task"))
        .then(|| TaskCreate::in_project(&project, q));
    Ok(view! {
        screen(project: &project, org: &org, tasks: &tasks, task_count: task_count, notes: Some(&notes), section: tab, q: q, task_create: create.as_ref())
    })
}

#[component]
async fn project_info(project: &Project) -> Result<impl View> {
    Ok(view! {
        info_columns(
            <div>
                description(text: &project.description)
                mono_value(id: "path-title", label: "Base path", value: &project.base_path)
                if !project.notes_path.is_empty() {
                    mono_value(id: "notes-path-title", label: "Notes path", value: &project.notes_path)
                }
            </div>
            settings_panel(s: project.settings())
        )
    })
}

#[component]
pub async fn screen(
    project: &Project,
    org: &Option<Organization>,
    tasks: &[Task],
    task_count: usize,
    section: Section,
    #[default] notes: Option<&NotesTab>,
    #[default] q: Option<&str>,
    #[default] task_create: Option<&TaskCreate>,
) -> Result<impl View> {
    let base = project_url(project.id());
    let mut items = section_tabs(
        &base,
        section,
        None,
        task_count,
        NotesTab::badge(notes),
        None,
    );
    items.push(settings_tab(&base, false));
    let parts = [section.title("Project"), project.name.as_str()];
    // The page has tabs, so the current tab is the current item; the crumb
    // is plain text, and with no organization the kicker is just a label.
    let crumbs = trail(org, None, Crumb::label("Project"));
    let rows = project_rows(tasks);
    let add = TaskHome::Project(project.id()).open_url(q);
    let dialog = task_create.map(|create| Child::new(view! { task_modal(create: create) }));
    let refused = task_create.is_some_and(|c| c.error.is_some());
    Ok(view! {
        frame(
            title: &parts,
            error: refused,
            dialog: dialog,
            page_header(title: &project.name, kicker: if org.is_some() { Kicker::Crumbs(&crumbs) } else { Kicker::Eyebrow("Project") })
            tabs(label: "Sections", items: &items)
            if section == Section::Tasks {
                tasks_tab(rows: &rows, caption: "Tasks in this project", total: task_count, search: search(&base, q), add: Some(add))

            } else if let (Section::Notes, Some(notes)) = (section, notes) {
                notes_section(base: &base, notes: notes)
            } else {
                project_info(project: project)
            }
        )
    })
}
