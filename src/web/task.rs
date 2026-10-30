//! The task page (`/task/{id}`), the sibling of the project page: the route,
//! the loading (`load`) and the view. Nothing here has rules of its own: it
//! is all shared components (`ui/`).

use super::Id;
use super::crumbs::trail;
use super::load::{Parents, parents_of};
use super::sections::{TabQuery, TaskSection, task_tabs};
use super::session_lines::task_lines;
use super::task_agent::{self, AgentTab, agent_tab};
use super::task_panel::task_panel;
use super::ui::breadcrumb::Crumb;
use super::ui::button::edit_button;
use super::ui::empty_line::empty_line;
use super::ui::frame::frame;
use super::ui::labelled::labelled_section;
use super::ui::layout::info_columns;
use super::ui::page_header::{Kicker, page_header};
use super::ui::prose::description;
use super::ui::sessions_table::sessions_table;
use super::ui::tabs::tabs;
use super::url::{task_edit_url, task_url};
use super::{now, open_db};
use crate::db::{Db, Table};
use crate::models::{Session, Tag, Task};
use chrono::NaiveDateTime;
use topcoat::{
    Result,
    context::Cx,
    router::{RouterBuilder, error::RouterErrorExt, page, path_param, query_params},
    view::{View, component, view},
};

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder.page(show)
}

/// What the task page shows.
pub struct Loaded {
    pub task: Task,
    pub parents: Parents,
    /// By start, oldest first (the database's order).
    pub sessions: Vec<Session>,
    /// By name.
    pub tags: Vec<Tag>,
}

/// Loads task `id`: its project, the project's organization (only when it
/// has one), its tags and its sessions. Five lookups by key or index, none
/// per row.
pub fn load(db: &Db, id: i64) -> Result<Loaded> {
    let task = db.get::<Task>(id)?.ok_or_not_found()?;
    let parents = parents_of(db, task.project_id)?;
    let sessions = db.sessions_for_task(id)?;
    let tags = db.tags_for_task(id)?;
    Ok(Loaded {
        task,
        parents,
        sessions,
        tags,
    })
}

#[page("/task/{id}")]
async fn show(cx: &Cx) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let query = query_params::<TabQuery>(cx)?;
    let section = TaskSection::of(query.tab.as_deref());
    let db = open_db()?;
    let page = load(&db, id)?;
    // the plan and the folder are read only for the tab that shows them
    let agent = match section {
        TaskSection::Agent => Some(task_agent::load(
            &db,
            &page.parents.project,
            &page.task,
            query.step.as_deref(),
        )?),
        _ => None,
    };
    let now = now();
    Ok(view! {
        screen(page: &page, section: section, now: now, agent: agent.as_ref())
    })
}

/// The Sessions block: the table, or "No sessions yet.". Oldest first, the order
/// the work happened in (the database's order): a log reads downward, and the
/// open session, if any, is the last row.
#[component]
async fn sessions_block(
    task: &Task,
    sessions: &[Session],
    now: NaiveDateTime,
) -> Result<impl View> {
    let lines = task_lines(sessions, now);
    let caption = format!("Sessions of {}", task.name);
    Ok(view! {
        labelled_section(id: "sessions-title", label: "Sessions", count: Some(lines.len()), noun: "sessions",
            if lines.is_empty() {
                empty_line("No sessions yet.")
            } else {
                sessions_table(lines: &lines, caption: &caption, show_end: true)
            }
        )
    })
}

/// The Info tab: the description, the Details aside.
#[component]
async fn task_info(task: &Task, tags: &[Tag]) -> Result<impl View> {
    Ok(view! {
        info_columns(
            <div>
                description(text: &task.description)
            </div>
            task_panel(task: task, tags: tags)
        )
    })
}

#[component]
pub async fn screen(
    page: &Loaded,
    section: TaskSection,
    now: NaiveDateTime,
    #[default] agent: Option<&AgentTab>,
) -> Result<impl View> {
    let Loaded {
        task,
        parents,
        sessions: rows,
        tags,
    } = page;
    let items = task_tabs(&task_url(task.id()), section, rows.len());
    // The page has tabs, so the current tab is the current item and the
    // page's own crumb is plain text.
    let crumbs = trail(&parents.org, Some(&parents.project), Crumb::label("Task"));
    let parts = [section.title("Task"), task.name.as_str()];
    Ok(view! {
        frame(
            title: &parts,
            page_header(title: &task.name, kicker: Kicker::Crumbs(&crumbs), edit_button(href: task_edit_url(task.id())))
            tabs(label: "Sections", items: &items)
            if section == TaskSection::Sessions {
                sessions_block(task: task, sessions: rows, now: now)
            } else if let (TaskSection::Agent, Some(agent)) = (section, agent) {
                agent_tab(task_id: task.id(), agent: agent)
            } else {
                task_info(task: task, tags: tags)
            }
        )
    })
}
