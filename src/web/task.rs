//! The task page (`/task/{id}`): route, loading and view, built from shared components.

use super::Id;
use super::crumbs::trail;
use super::load::{Parents, parents_of};
use super::sections::{TabQuery, TaskSection, task_tabs};
use super::session_lines::task_lines;
use super::task_agent::{self, AgentTab, agent_tab, files_block, plan_block};
use super::task_notes::note_modal;
use super::task_panel::task_panel;
use super::task_runs::compose_modal;
use super::task_sessions::session_modal;
use super::task_wiki::{self, WikiTab, wiki_tab};
use super::ui::breadcrumb::Crumb;
use super::ui::frame::frame;
use super::ui::layout::info_columns;
use super::ui::page_header::{Kicker, page_header};
use super::ui::prose::description;
use super::ui::sessions_table::sessions_table;
use super::ui::table::AddRow;
use super::ui::tabs::tabs;
use super::url::{notes_tab_url, page_url, sessions_tab_url, task_url};
use super::{now, open_db};
use crate::agent::taskdir;
use crate::db::{Db, Table};
use crate::models::{Session, Tag, Task};
use chrono::NaiveDateTime;
use topcoat::{
    Result,
    context::Cx,
    router::{RouterBuilder, error::RouterErrorExt, page, path_param, query_params},
    view::{Child, View, component, view},
};

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder.page(show)
}

pub struct Loaded {
    pub task: Task,
    pub parents: Parents,
    pub sessions: Vec<Session>,
    pub tags: Vec<Tag>,
}

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
    let agent = match section {
        TaskSection::Agent => {
            // the prompts of a new agent are made with the commands' own code, which wants an app
            let app = crate::app::App {
                db: open_db()?,
                now: chrono::Local::now().naive_local(),
            };
            let open = task_agent::Open {
                step: query.step.as_deref(),
                run: query.run.as_deref(),
                new: query.new.as_deref(),
                instruction: query.instruction.as_deref(),
            };
            Some(task_agent::load(
                &app,
                &page.parents.project,
                &page.task,
                &open,
            )?)
        }
        _ => None,
    };
    let wiki = match section {
        TaskSection::Wiki => Some(task_wiki::load(&db, &page.parents.project, &page.task)),
        _ => None,
    };
    let files = match section {
        TaskSection::Notes | TaskSection::Wiki => {
            Some(task_agent::files(&db, &page.parents.project, &page.task))
        }
        _ => None,
    };
    let plan = (section == TaskSection::Info)
        .then(|| task_agent::plan_of(&db, &page.parents.project, &page.task));
    let agents = taskdir::of(&db, &page.parents.project, &page.task)
        .map(|d| d.runs().len())
        .unwrap_or(0);
    let add_session = section == TaskSection::Sessions && query.new.as_deref() == Some("session");
    let add_note = section == TaskSection::Notes && query.new.as_deref() == Some("note");
    let now = now();
    Ok(view! {
        screen(page: &page, section: section, now: now, agent: agent.as_ref(), wiki: wiki.as_ref(), files: files.as_ref(), add_session: add_session, add_note: add_note, plan: plan.as_ref(), agents: agents)
    })
}

/// Oldest first, the order the work happened in: the open session, if any, is the last row.
#[component]
async fn sessions_block(
    task: &Task,
    sessions: &[Session],
    now: NaiveDateTime,
) -> Result<impl View> {
    let lines = task_lines(sessions, now);
    let caption = format!("Sessions of {}", task.name);
    let add = page_url(
        &sessions_tab_url(&task_url(task.id())),
        &[("new", Some("session"))],
    );
    Ok(view! {
        sessions_table(lines: &lines, caption: &caption, show_end: true, add: Some(add), empty: lines.is_empty().then_some("No sessions yet."))
    })
}

#[component]
async fn task_info(
    task: &Task,
    tags: &[Tag],
    plan: Option<&crate::agent::plan::Plan>,
) -> Result<impl View> {
    Ok(view! {
        info_columns(
            <div>
                description(text: &task.description)
                plan_block(task_id: task.id(), plan: plan)
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
    #[default] wiki: Option<&WikiTab>,
    #[default] files: Option<&task_agent::Files>,
    #[default] add_session: bool,
    #[default] add_note: bool,
    #[default] plan: Option<&crate::agent::plan::Plan>,
    #[default] agents: usize,
) -> Result<impl View> {
    let Loaded {
        task,
        parents,
        sessions: rows,
        tags,
    } = page;
    let items = task_tabs(&task_url(task.id()), section, rows.len(), agents);
    let new_note = page_url(
        &notes_tab_url(&task_url(task.id())),
        &[("new", Some("note"))],
    );
    // The page has tabs, so the current tab is the current item and the
    // page's own crumb is plain text.
    let crumbs = trail(&parents.org, Some(&parents.project), Crumb::label("Task"));
    let parts = [section.title("Task"), task.name.as_str()];
    let dialog = if add_session && section == TaskSection::Sessions {
        Some(Child::new(view! { session_modal(task_id: task.id()) }))
    } else if add_note && section == TaskSection::Notes {
        Some(Child::new(view! { note_modal(task_id: task.id()) }))
    } else {
        agent
            .and_then(|a| a.roster.compose.as_ref())
            .map(|c| Child::new(view! { compose_modal(task_id: task.id(), c: c) }))
    };
    Ok(view! {
        frame(
            title: &parts,
            dialog: dialog,
            page_header(title: &task.name, kicker: Kicker::Crumbs(&crumbs))
            tabs(label: "Sections", items: &items)
            if section == TaskSection::Sessions {
                sessions_block(task: task, sessions: rows, now: now)
            } else if let (TaskSection::Agent, Some(agent)) = (section, agent) {
                agent_tab(task_id: task.id(), agent: agent)
            } else if let (TaskSection::Notes, Some(files)) = (section, files) {
                files_block(label: "Notes", noun: "notes", docs: &files.notes, add: Some(AddRow::link(&new_note, "New note\u{2026}")))
            } else if let (TaskSection::Wiki, Some(wiki), Some(files)) = (section, wiki, files) {
                wiki_tab(tab: wiki, sketches: &files.sketches)
            } else {
                task_info(task: task, tags: tags, plan: plan)
            }
        )
    })
}
