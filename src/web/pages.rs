//! The pages: organization, project and task detail, and an edit form for
//! each. Every handler reads what it needs into plain owned data first and
//! only then builds its view, so no database connection lives across
//! rendering.

use super::edit::{EditForm, Loaded, load, submit};
use super::forms::{OrgForm, ProjectForm, TaskForm, joined, names};
use super::layout::{
    self, Nav, Sel, check, cls, description, error_box, field, form_actions, shell, textarea,
};
use super::open_db;
use super::org;
use super::v2::compare;
use crate::config::config;
use crate::db::{Db, Table};
use crate::git;
use crate::models::{
    Configured, Duration, Organization, Project, ProjectGroup, START_TIME_FMT, Session, Settings,
    Tag, Task, TaskStatus,
};
use chrono::{Local, NaiveDateTime};
use topcoat::{
    Result,
    context::Cx,
    router::{
        RouterBuilder,
        content::Form,
        error::{RouterErrorExt, see_other},
        page, path_param, query_params,
    },
    view::{View, component, view},
};

path_param!(id: i64, error = bad_request);

#[query_params(error = bad_request)]
struct TabQuery {
    tab: Option<String>,
    /// `old` renders a redesigned page's old design (see `v2.rs`).
    design: Option<String>,
    /// The organization report's period (`org::load_report`).
    from: Option<String>,
    to: Option<String>,
}

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder
        .layout(layout::root)
        .route(layout::stylesheet)
        .route(layout::board_script)
        .page(org_page)
        .page(org_edit)
        .page(org_save)
        .page(project_page)
        .page(project_edit)
        .page(project_save)
        .page(task_page)
        .page(task_edit)
        .page(task_save)
        .page(task_new)
        .page(task_new_save)
}

// ---- small building blocks --------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Info,
    Tasks,
}

impl Tab {
    fn of(cx: &Cx) -> Result<Tab> {
        Ok(match query_params::<TabQuery>(cx)?.tab.as_deref() {
            Some("tasks") => Tab::Tasks,
            _ => Tab::Info,
        })
    }
}

fn yes_no(on: bool) -> &'static str {
    if on { "yes" } else { "no" }
}

fn issue(number: Option<i64>) -> String {
    number.map(|n| format!("#{n}")).unwrap_or_default()
}

/// The Info | Tasks switch, and the entity's edit link.
#[component]
async fn tabs(base: &str, tab: Tab) -> Result<impl View> {
    Ok(view! {
        <div class="tabs">
            <a class=(cls(tab == Tab::Info)) href=(base.to_string())>"Info"</a>
            <a class=(cls(tab == Tab::Tasks)) href=(format!("{base}?tab=tasks"))>"Tasks"</a>
            <a class="edit" href=(format!("{base}/edit"))>"Edit"</a>
        </div>
    })
}

#[component]
async fn row(label: &str, value: &str) -> Result<impl View> {
    Ok(view! { <dt>(label.to_string())</dt><dd>(value.to_string())</dd> })
}

/// The branch/tool settings an organization and a project share.
#[component]
async fn settings(s: Settings) -> Result<impl View> {
    Ok(view! {
        <dl>
            row(label: "github", value: yes_no(s.github))
            row(label: "tmux", value: yes_no(s.tmux))
            row(label: "auto branch", value: yes_no(s.auto_branch))
            row(label: "branch template", value: &s.branch_template)
            row(label: "default branch", value: &s.default_branch)
            row(label: "github project", value: &s.github_project)
        </dl>
    })
}

/// The inputs a `SettingsForm` reads back.
#[component]
async fn settings_fields(s: Settings) -> Result<impl View> {
    Ok(view! {
        check(name: "github", label: "github", on: s.github)
        check(name: "tmux", label: "tmux", on: s.tmux)
        check(name: "auto_branch", label: "auto branch", on: s.auto_branch)
        field(name: "branch_template", label: "Branch template", value: &s.branch_template)
        field(name: "default_branch", label: "Default branch", value: &s.default_branch)
        field(name: "github_project", label: "GitHub project", value: &s.github_project)
    })
}

/// The organization `project` is in, if any.
fn org_of(db: &Db, project: &Project) -> crate::error::Result<Option<Organization>> {
    Ok(match Organization::group_of(project) {
        Some(id) => db.get(id)?,
        None => None,
    })
}

/// `rows` are `(project name, task)`, the shape `Db::tasks_in` returns.
#[component]
async fn task_table(
    rows: &[(String, Task)],
    with_project: bool,
    /// The project a placeholder "new task" row at the top adds to, if any.
    #[default] new_in: Option<i64>,
) -> Result<impl View> {
    Ok(view! {
        if rows.is_empty() && new_in.is_none() {
            <p class="empty">"No tasks."</p>
        } else {
            <table>
                <tr>
                    <th>"Task"</th>
                    if with_project {
                        <th>"Project"</th>
                    }
                    <th>"Status"</th>
                    <th>"Issue"</th>
                </tr>
                if let Some(project_id) = new_in {
                    <tr class="new">
                        <td colspan=(if with_project { "4" } else { "3" })>
                            <a href=(format!("/project/{project_id}/task/new"))>"+ New task"</a>
                        </td>
                    </tr>
                }
                for (project, task) in rows.iter() {
                    <tr>
                        <td><a href=(format!("/task/{}", task.id()))>(task.name.clone())</a></td>
                        if with_project {
                            <td><a href=(format!("/project/{}", task.project_id))>(project.clone())</a></td>
                        }
                        <td>
                            <span class=(format!("pill {}", task.status.as_str()))>(task.status.label())</span>
                        </td>
                        <td>(issue(task.github_issue))</td>
                    </tr>
                }
            </table>
        }
    })
}

/// The breadcrumb's leading `Organization / ` when there is one.
#[component]
async fn org_crumb(org: &Option<Organization>) -> Result<impl View> {
    Ok(view! {
        if let Some(org) = org {
            <a href=(format!("/org/{}", org.id()))>(org.name.clone())</a>" / "
        }
    })
}

// ---- organizations ----------------------------------------------------------

#[page("/org/{id}")]
async fn org_page(cx: &Cx) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let tab = Tab::of(cx)?;
    let query = query_params::<TabQuery>(cx)?;
    let old = super::v2::is_old(&query.design);
    // The Report tab is the proposal's own; the old design shows Info there.
    let section = match query.tab.as_deref() {
        Some("report") if !old => org::Section::Report,
        _ if tab == Tab::Tasks => org::Section::Tasks,
        _ => org::Section::Info,
    };
    let now = Local::now().naive_local();
    let (nav, org, projects, tasks, report) = {
        let db = open_db()?;
        let org = db.get::<Organization>(id)?.ok_or_not_found()?;
        let projects = db.projects_in::<Organization>(id)?;
        // The proposal counts them on its Tasks tab, so it needs them on both.
        let tasks = if tab == Tab::Tasks || !old {
            db.tasks_in::<Organization>(id)?
        } else {
            Vec::new()
        };
        let report = match section {
            org::Section::Report => Some(org::load_report(
                &db,
                id,
                query.from.as_deref(),
                query.to.as_deref(),
                now,
            )?),
            _ => None,
        };
        (Nav::load(&db)?, org, projects, tasks, report)
    };
    let base = format!("/org/{id}");
    // The page this is, as the proposal sees it -- built from the query,
    // not from `section`, so the old design (which has no Report tab)
    // still switches back to the report it came from.
    let here = match query.tab.as_deref() {
        Some("report") => match (&query.from, &query.to) {
            (None, None) => format!("{base}?tab=report"),
            (from, to) => format!(
                "{base}?tab=report&from={}&to={}",
                from.as_deref().unwrap_or_default(),
                to.as_deref().unwrap_or_default()
            ),
        },
        _ if tab == Tab::Tasks => format!("{base}?tab=tasks"),
        _ => base.clone(),
    };
    Ok(view! {
        if old {
            shell(
                nav: &nav,
                sel: Sel::Org(id),
                <div class="crumbs">"Organization"</div>
                <h1>(org.name.clone())</h1>
                tabs(base: &base, tab: tab)
                if tab == Tab::Info {
                    description(text: &org.description)
                    settings(s: org.settings())
                    <h3>"Projects"</h3>
                    if projects.is_empty() {
                        <p class="empty">"No projects."</p>
                    }
                    <ul>
                        for p in projects.iter() {
                            <li><a href=(format!("/project/{}", p.id()))>(p.name.clone())</a></li>
                        }
                    </ul>
                } else {
                    task_table(rows: &tasks, with_project: true)
                }
                compare(old: true, here: here.clone())
            )
        } else {
            org::proposal(org: &org, projects: &projects, tasks: &tasks, section: section, report: &report, today: now.date())
            compare(old: false, here: here.clone())
        }
    })
}

#[component]
async fn org_form(
    org: &Organization,
    projects: &str,
    #[default] error: Option<String>,
) -> Result<impl View> {
    Ok(view! {
        <h1>"Edit organization"</h1>
        error_box(error: &error)
        <form method="post" action=(format!("/org/{}/edit", org.id()))>
            field(name: "name", label: "Name", value: &org.name)
            textarea(text: &org.description)
            settings_fields(s: org.settings())
            field(name: "projects", label: "Projects (comma-separated)", value: projects)
            form_actions(cancel: format!("/org/{}", org.id()))
        </form>
    })
}

#[page("/org/{id}/edit")]
async fn org_edit(cx: &Cx) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let page: Loaded<Organization, _> =
        load(id, |db| Ok(joined(&db.projects_in::<Organization>(id)?)))?;
    Ok(view! {
        shell(nav: &page.nav, sel: Sel::Org(id), org_form(org: &page.row, projects: &page.extra))
    })
}

#[page(POST "/org/{id}/edit")]
async fn org_save(cx: &Cx, Form(form): Form<OrgForm>) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let (page, error) = submit(id, &form, |_| Ok(()))?;
    Ok(view! {
        shell(nav: &page.nav, sel: Sel::Org(id), org_form(org: &page.row, projects: &form.projects, error: Some(error)))
    })
}

// ---- projects ---------------------------------------------------------------

#[page("/project/{id}")]
async fn project_page(cx: &Cx) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let tab = Tab::of(cx)?;
    let (nav, project, org, tasks) = {
        let db = open_db()?;
        let project = db.get::<Project>(id)?.ok_or_not_found()?;
        let org = org_of(&db, &project)?;
        let tasks = db
            .tasks_for_project(id)?
            .into_iter()
            .map(|task| (project.name.clone(), task))
            .collect::<Vec<_>>();
        (Nav::load(&db)?, project, org, tasks)
    };
    let base = format!("/project/{id}");
    Ok(view! {
        shell(
            nav: &nav,
            sel: Sel::Project(id),
            <div class="crumbs">org_crumb(org: &org) "Project"</div>
            <h1>(project.name.clone())</h1>
            tabs(base: &base, tab: tab)
            if tab == Tab::Info {
                description(text: &project.description)
                <dl>row(label: "base path", value: &project.base_path)</dl>
                settings(s: project.settings())
            } else {
                task_table(rows: &tasks, with_project: false, new_in: Some(id))
            }
        )
    })
}

#[component]
async fn project_form(
    project: &Project,
    orgs: &[Organization],
    #[default] error: Option<String>,
) -> Result<impl View> {
    let current = project.organization_id;
    Ok(view! {
        <h1>"Edit project"</h1>
        error_box(error: &error)
        <form method="post" action=(format!("/project/{}/edit", project.id()))>
            field(name: "name", label: "Name", value: &project.name)
            textarea(text: &project.description)
            field(name: "base_path", label: "Base path", value: &project.base_path)
            <label>"Organization"</label>
            <select name="organization">
                <option value="" if current.is_none() { selected="" }>"(none)"</option>
                for o in orgs.iter() {
                    <option value=(o.id().to_string()) if current == Some(o.id()) { selected="" }>
                        (o.name.clone())
                    </option>
                }
            </select>
            settings_fields(s: project.settings())
            form_actions(cancel: format!("/project/{}", project.id()))
        </form>
    })
}

#[page("/project/{id}/edit")]
async fn project_edit(cx: &Cx) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let page: Loaded<Project, _> = load(id, |db| db.list::<Organization>())?;
    Ok(view! {
        shell(nav: &page.nav, sel: Sel::Project(id), project_form(project: &page.row, orgs: &page.extra))
    })
}

#[page(POST "/project/{id}/edit")]
async fn project_save(cx: &Cx, Form(form): Form<ProjectForm>) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let (page, error) = submit(id, &form, |db| db.list::<Organization>())?;
    Ok(view! {
        shell(
            nav: &page.nav,
            sel: Sel::Project(id),
            project_form(project: &page.row, orgs: &page.extra, error: Some(error))
        )
    })
}

// ---- tasks ------------------------------------------------------------------

fn stamp(t: NaiveDateTime) -> String {
    t.format(START_TIME_FMT).to_string()
}

#[component]
async fn sessions_table(sessions: &[Session]) -> Result<impl View> {
    let now = Local::now().naive_local();
    Ok(view! {
        <h3>"Sessions"</h3>
        if sessions.is_empty() {
            <p class="empty">"No sessions."</p>
        } else {
            <table>
                <tr><th>"Start"</th><th>"End"</th><th>"Time"</th><th>"Message"</th></tr>
                for s in sessions.iter() {
                    <tr>
                        <td>(stamp(s.start))</td>
                        <td>(s.end.map(stamp).unwrap_or_else(|| "ongoing".to_string()))</td>
                        <td>(Duration(s.duration_minutes(now)).to_string())</td>
                        <td>(s.message.clone().unwrap_or_default())</td>
                    </tr>
                }
            </table>
        }
    })
}

#[page("/task/{id}")]
async fn task_page(cx: &Cx) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let (nav, task, project, org, sessions, tags) = {
        let db = open_db()?;
        let task = db.get::<Task>(id)?.ok_or_not_found()?;
        let project = db.get::<Project>(task.project_id)?.ok_or_not_found()?;
        let org = org_of(&db, &project)?;
        let sessions = db.sessions_for_task(id)?;
        let tags = db.tags_for_task(id)?;
        (Nav::load(&db)?, task, project, org, sessions, tags)
    };
    let issue_text = issue(task.github_issue);
    let (start_text, duration_text) = (task.start_text(), task.duration_text());
    Ok(view! {
        shell(
            nav: &nav,
            sel: Sel::Project(project.id()),
            <div class="crumbs">
                org_crumb(org: &org)
                <a href=(format!("/project/{}?tab=tasks", project.id()))>(project.name.clone())</a>
                " / Task"
            </div>
            <h1>(task.name.clone())</h1>
            <div class="tabs">
                <a class="sel" href=(format!("/task/{id}"))>"Info"</a>
                <a class="edit" href=(format!("/task/{id}/edit"))>"Edit"</a>
            </div>
            description(text: &task.description)
            <dl>
                row(label: "status", value: task.status.label())
                row(label: "github issue", value: &issue_text)
                row(label: "branch prefix", value: &task.branch_prefix)
                row(label: "start", value: &start_text)
                row(label: "duration", value: &duration_text)
                <dt>"tags"</dt>
                <dd>
                    for t in tags.iter() {
                        <span class="chip" style=(format!("background:{}", t.color))>(t.name.clone())</span>
                    }
                </dd>
            </dl>
            sessions_table(sessions: &sessions)
        )
    })
}

#[component]
async fn task_form(
    task: &Task,
    tags: &str,
    title: &str,
    #[into] action: String,
    #[into] cancel: String,
    #[default] error: Option<String>,
) -> Result<impl View> {
    let issue_number = task.github_issue.map(|n| n.to_string()).unwrap_or_default();
    let (start, duration) = (task.start_text(), task.duration_text());
    Ok(view! {
        <h1>(title.to_string())</h1>
        error_box(error: &error)
        <form method="post" action=(action.clone())>
            field(name: "name", label: "Name", value: &task.name)
            textarea(text: &task.description)
            <label>"Status"</label>
            <select name="status">
                for s in TaskStatus::ALL {
                    <option value=(s.as_str()) if task.status == s { selected="" }>(s.label())</option>
                }
            </select>
            field(name: "github_issue", label: "GitHub issue number", value: &issue_number)
            field(name: "branch_prefix", label: "Branch prefix", value: &task.branch_prefix)
            field(name: "start_time", label: "Start (yyyy-mm-dd hh:mm)", value: &start)
            field(name: "duration", label: "Duration (hh:mm)", value: &duration)
            field(name: "tags", label: "Tags (comma-separated -- Urgent and Important flag the task)", value: tags)
            form_actions(cancel: cancel.clone())
        </form>
    })
}

#[page("/task/{id}/edit")]
async fn task_edit(cx: &Cx) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let page: Loaded<Task, _> = load(id, |db| Ok(joined(&db.tags_for_task(id)?)))?;
    Ok(view! {
        shell(nav: &page.nav, sel: Sel::Project(page.row.project_id), task_form(
            task: &page.row,
            tags: &page.extra,
            title: "Edit task",
            action: format!("/task/{id}/edit"),
            cancel: format!("/task/{id}"),
        ))
    })
}

#[page(POST "/task/{id}/edit")]
async fn task_save(cx: &Cx, Form(form): Form<TaskForm>) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let (page, error) = submit(id, &form, |_| Ok(()))?;
    Ok(view! {
        shell(
            nav: &page.nav,
            sel: Sel::Project(page.row.project_id),
            task_form(
                task: &page.row,
                tags: &form.tags,
                title: "Edit task",
                action: format!("/task/{id}/edit"),
                cancel: format!("/task/{id}"),
                error: Some(error),
            )
        )
    })
}

#[page("/project/{id}/task/new")]
async fn task_new(cx: &Cx) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let (nav, task) = {
        let db = open_db()?;
        let project = db.get::<Project>(id)?.ok_or_not_found()?;
        let task = Task::template(id, git::branch_prefix(&project.branch_template), config().task.status);
        (Nav::load(&db)?, task)
    };
    Ok(view! {
        shell(
            nav: &nav,
            sel: Sel::Project(id),
            task_form(
                task: &task,
                tags: "",
                title: "New task",
                action: format!("/project/{id}/task/new"),
                cancel: format!("/project/{id}?tab=tasks"),
            )
        )
    })
}

#[page(POST "/project/{id}/task/new")]
async fn task_new_save(cx: &Cx, Form(form): Form<TaskForm>) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let (nav, task, error) = {
        let db = open_db()?;
        let project = db.get::<Project>(id)?.ok_or_not_found()?;
        let template = Task::template(id, git::branch_prefix(&project.branch_template), config().task.status);
        let (task, valid) = form.apply(template);
        let saved = valid
            .and_then(|()| db.ids_by_name::<Tag>(&names(&form.tags)))
            .and_then(|tag_ids| {
                let new_id = db.insert(&task)?;
                db.set_task_tags(new_id, &tag_ids)?;
                Ok(new_id)
            });
        match saved {
            Ok(new_id) => return Err(see_other(format!("/task/{new_id}")).into()),
            Err(e) => (Nav::load(&db)?, task, e.to_string()),
        }
    };
    Ok(view! {
        shell(
            nav: &nav,
            sel: Sel::Project(id),
            task_form(
                task: &task,
                tags: &form.tags,
                title: "New task",
                action: format!("/project/{id}/task/new"),
                cancel: format!("/project/{id}?tab=tasks"),
                error: Some(error),
            )
        )
    })
}
