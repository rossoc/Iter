//! The task page (`/task/{id}`), the sibling of the project page: the route,
//! the loading (`load`) and the view. Nothing here has rules of its own: it
//! is all shared components (`ui/`).

use super::Id;
use super::crumbs::trail;
use super::load::{Parents, parents_of};
use super::session_lines::task_lines;
use super::task_panel::task_panel;
use super::ui::breadcrumb::Crumb;
use super::ui::button::edit_button;
use super::ui::columns::info_columns;
use super::ui::empty_line::empty_line;
use super::ui::frame::frame;
use super::ui::labelled::labelled_section;
use super::ui::page_header::{Kicker, page_header};
use super::ui::prose::description;
use super::ui::sessions_table::sessions_table;
use super::url::task_edit_url;
use super::{now, open_db};
use crate::db::{Db, Table};
use crate::models::{Session, Tag, Task};
use chrono::NaiveDateTime;
use topcoat::{
    Result,
    context::Cx,
    router::{RouterBuilder, error::RouterErrorExt, page, path_param},
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
    let page = load(&open_db()?, id)?;
    let now = now();
    Ok(view! {
        screen(page: &page, now: now)
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

#[component]
pub async fn screen(page: &Loaded, now: NaiveDateTime) -> Result<impl View> {
    let Loaded {
        task,
        parents,
        sessions: rows,
        tags,
    } = page;
    // No tabs, so the page's own crumb is the current item.
    let crumbs = trail(&parents.org, Some(&parents.project), Crumb::here("Task"));
    let parts = ["Task", task.name.as_str()];
    Ok(view! {
        frame(
            title: &parts,
            page_header(title: &task.name, kicker: Kicker::Crumbs(&crumbs), edit_button(href: task_edit_url(task.id())))
            info_columns(
                <div>
                    description(text: &task.description)
                    sessions_block(task: task, sessions: rows, now: now)
                </div>
                task_panel(task: task, tags: tags)
            )
        )
    })
}
