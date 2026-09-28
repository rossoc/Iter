//! The pages: organization, project and task detail, and an edit form for
//! each. Every handler reads what it needs into plain owned data first and
//! only then builds its view, so no database connection lives across
//! rendering.

use super::forms::{OrgForm, ProjectForm, TaskForm};
use super::layout::{self, Nav, Sel, shell};
use super::open_db;
use crate::db::{Db, Table};
use crate::models::{Organization, Project, START_TIME_FMT, Session, Tag, Task, TaskStatus};
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
}

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder
        .layout(layout::root)
        .route(layout::stylesheet)
        .route(layout::board_script)
        .page(home)
        .page(org_page)
        .page(org_edit)
        .page(org_save)
        .page(project_page)
        .page(project_edit)
        .page(project_save)
        .page(task_page)
        .page(task_edit)
        .page(task_save)
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

fn cls(on: bool) -> &'static str {
    if on { "sel" } else { "" }
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

#[component]
async fn description(text: &str) -> Result<impl View> {
    Ok(view! {
        if !text.is_empty() {
            <pre class="desc">(text.to_string())</pre>
        }
    })
}

/// The branch/tool settings an organization and a project share.
#[component]
async fn settings(
    github: bool,
    tmux: bool,
    auto_branch: bool,
    branch_template: &str,
    default_branch: &str,
    github_project: &str,
) -> Result<impl View> {
    Ok(view! {
        <dl>
            row(label: "github", value: yes_no(github))
            row(label: "tmux", value: yes_no(tmux))
            row(label: "auto branch", value: yes_no(auto_branch))
            row(label: "branch template", value: branch_template)
            row(label: "default branch", value: default_branch)
            row(label: "github project", value: github_project)
        </dl>
    })
}

/// A task with the project it belongs to, for listings that span projects.
struct TaskRow {
    project: Project,
    task: Task,
}

fn tasks_of(db: &Db, project: &Project) -> crate::error::Result<Vec<TaskRow>> {
    Ok(db
        .tasks_for_project(project.id())?
        .into_iter()
        .map(|task| TaskRow {
            project: project.clone(),
            task,
        })
        .collect())
}

#[component]
async fn task_table(rows: &[TaskRow], with_project: bool) -> Result<impl View> {
    Ok(view! {
        if rows.is_empty() {
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
                for r in rows.iter() {
                    <tr>
                        <td><a href=(format!("/task/{}", r.task.id()))>(r.task.name.clone())</a></td>
                        if with_project {
                            <td><a href=(format!("/project/{}", r.project.id()))>(r.project.name.clone())</a></td>
                        }
                        <td>
                            <span class=(format!("pill {}", r.task.status.as_str()))>(r.task.status.label())</span>
                        </td>
                        <td>(issue(r.task.github_issue))</td>
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

#[component]
async fn error_box(error: &Option<String>) -> Result<impl View> {
    Ok(view! {
        if let Some(message) = error {
            <div class="error">(message.clone())</div>
        }
    })
}

#[component]
async fn check(name: &str, label: &str, on: bool) -> Result<impl View> {
    Ok(view! {
        <label class="check">
            <input type="checkbox" name=(name.to_string()) if on { checked="" }>
            (label.to_string())
        </label>
    })
}

#[component]
async fn field(name: &str, label: &str, value: &str) -> Result<impl View> {
    Ok(view! {
        <label>(label.to_string())</label>
        <input type="text" name=(name.to_string()) value=(value.to_string())>
    })
}

#[component]
async fn textarea(text: &str) -> Result<impl View> {
    Ok(view! {
        <label>"Description (markdown)"</label>
        <textarea name="description">(text.to_string())</textarea>
    })
}

#[component]
async fn form_actions(#[into] cancel: String) -> Result<impl View> {
    Ok(view! {
        <button type="submit">"Save"</button>
        " "
        <a href=(cancel.clone())>"Cancel"</a>
    })
}

// ---- home -------------------------------------------------------------------

#[page("/")]
async fn home() -> Result<impl View> {
    let nav = Nav::load(&open_db()?)?;
    Ok(view! {
        shell(
            nav: &nav,
            sel: Sel::None,
            <p class="empty">"Select an organization or a project."</p>
        )
    })
}

// ---- organizations ----------------------------------------------------------

#[page("/org/{id}")]
async fn org_page(cx: &Cx) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let tab = Tab::of(cx)?;
    let (nav, org, projects, tasks) = {
        let db = open_db()?;
        let org = db.get::<Organization>(id)?.ok_or_not_found()?;
        let projects = db.projects_for_organization(id)?;
        let mut tasks = Vec::new();
        if tab == Tab::Tasks {
            for p in &projects {
                tasks.extend(tasks_of(&db, p)?);
            }
        }
        (Nav::load(&db)?, org, projects, tasks)
    };
    let base = format!("/org/{id}");
    Ok(view! {
        shell(
            nav: &nav,
            sel: Sel::Org(id),
            <div class="crumbs">"Organization"</div>
            <h1>(org.name.clone())</h1>
            tabs(base: &base, tab: tab)
            if tab == Tab::Info {
                description(text: &org.description)
                settings(
                    github: org.github,
                    tmux: org.tmux,
                    auto_branch: org.auto_branch,
                    branch_template: &org.branch_template,
                    default_branch: &org.default_branch,
                    github_project: &org.github_project,
                )
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
        )
    })
}

#[component]
async fn org_form(org: &Organization, #[default] error: Option<String>) -> Result<impl View> {
    Ok(view! {
        <h1>"Edit organization"</h1>
        error_box(error: &error)
        <form method="post" action=(format!("/org/{}/edit", org.id()))>
            field(name: "name", label: "Name", value: &org.name)
            textarea(text: &org.description)
            check(name: "github", label: "github", on: org.github)
            check(name: "tmux", label: "tmux", on: org.tmux)
            check(name: "auto_branch", label: "auto branch", on: org.auto_branch)
            field(name: "branch_template", label: "Branch template", value: &org.branch_template)
            field(name: "default_branch", label: "Default branch", value: &org.default_branch)
            field(name: "github_project", label: "GitHub project", value: &org.github_project)
            form_actions(cancel: format!("/org/{}", org.id()))
        </form>
    })
}

#[page("/org/{id}/edit")]
async fn org_edit(cx: &Cx) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let (nav, org) = {
        let db = open_db()?;
        let org = db.get::<Organization>(id)?.ok_or_not_found()?;
        (Nav::load(&db)?, org)
    };
    Ok(view! { shell(nav: &nav, sel: Sel::Org(id), org_form(org: &org)) })
}

#[page(POST "/org/{id}/edit")]
async fn org_save(cx: &Cx, Form(form): Form<OrgForm>) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let (nav, org, error) = {
        let db = open_db()?;
        let existing = db.get::<Organization>(id)?.ok_or_not_found()?;
        let (org, valid) = form.apply(existing);
        match valid.and_then(|()| db.update(id, &org)) {
            Ok(()) => return Err(see_other(format!("/org/{id}")).into()),
            Err(e) => (Nav::load(&db)?, org, e.to_string()),
        }
    };
    Ok(view! { shell(nav: &nav, sel: Sel::Org(id), org_form(org: &org, error: Some(error))) })
}

// ---- projects ---------------------------------------------------------------

#[page("/project/{id}")]
async fn project_page(cx: &Cx) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let tab = Tab::of(cx)?;
    let (nav, project, org, tasks) = {
        let db = open_db()?;
        let project = db.get::<Project>(id)?.ok_or_not_found()?;
        let org = match project.organization_id {
            Some(org_id) => db.get::<Organization>(org_id)?,
            None => None,
        };
        let tasks = tasks_of(&db, &project)?;
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
                settings(
                    github: project.github,
                    tmux: project.tmux,
                    auto_branch: project.auto_branch,
                    branch_template: &project.branch_template,
                    default_branch: &project.default_branch,
                    github_project: &project.github_project,
                )
            } else {
                task_table(rows: &tasks, with_project: false)
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
            check(name: "github", label: "github", on: project.github)
            check(name: "tmux", label: "tmux", on: project.tmux)
            check(name: "auto_branch", label: "auto branch", on: project.auto_branch)
            field(name: "branch_template", label: "Branch template", value: &project.branch_template)
            field(name: "default_branch", label: "Default branch", value: &project.default_branch)
            field(name: "github_project", label: "GitHub project", value: &project.github_project)
            form_actions(cancel: format!("/project/{}", project.id()))
        </form>
    })
}

#[page("/project/{id}/edit")]
async fn project_edit(cx: &Cx) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let (nav, project, orgs) = {
        let db = open_db()?;
        let project = db.get::<Project>(id)?.ok_or_not_found()?;
        (Nav::load(&db)?, project, db.list::<Organization>()?)
    };
    Ok(view! { shell(nav: &nav, sel: Sel::Project(id), project_form(project: &project, orgs: &orgs)) })
}

#[page(POST "/project/{id}/edit")]
async fn project_save(cx: &Cx, Form(form): Form<ProjectForm>) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let (nav, project, orgs, error) = {
        let db = open_db()?;
        let existing = db.get::<Project>(id)?.ok_or_not_found()?;
        let (project, valid) = form.apply(existing);
        match valid.and_then(|()| db.update(id, &project)) {
            Ok(()) => return Err(see_other(format!("/project/{id}")).into()),
            Err(e) => (Nav::load(&db)?, project, db.list::<Organization>()?, e.to_string()),
        }
    };
    Ok(view! {
        shell(
            nav: &nav,
            sel: Sel::Project(id),
            project_form(project: &project, orgs: &orgs, error: Some(error))
        )
    })
}

// ---- tasks ------------------------------------------------------------------

fn minutes(total: i64) -> String {
    format!("{}h {:02}m", total / 60, total % 60)
}

fn stamp(t: NaiveDateTime) -> String {
    t.format("%Y-%m-%d %H:%M").to_string()
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
                        <td>(minutes(s.duration_minutes(now)))</td>
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
        let org = match project.organization_id {
            Some(org_id) => db.get::<Organization>(org_id)?,
            None => None,
        };
        let sessions = db.sessions_for_task(id)?;
        let tags = db.tags_for_task(id)?;
        (Nav::load(&db)?, task, project, org, sessions, tags)
    };
    let issue_text = issue(task.github_issue);
    let start_text = task
        .start_time
        .map(|t| t.format(START_TIME_FMT).to_string())
        .unwrap_or_default();
    let duration_text = task.duration.map(|d| d.to_string()).unwrap_or_default();
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
                row(label: "urgent", value: yes_no(task.urgency))
                row(label: "important", value: yes_no(task.importance))
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

fn tag_list(tags: &[Tag]) -> String {
    tags.iter().map(|t| t.name.as_str()).collect::<Vec<_>>().join(", ")
}

/// The ids of the tags named `names`; an unknown name is an error rather
/// than a new tag, so a typo can't quietly mint one.
fn tag_ids(db: &Db, names: &[String]) -> crate::error::Result<Vec<i64>> {
    names
        .iter()
        .map(|n| {
            db.find_by_name::<Tag>(n)?
                .map(|t| t.id())
                .ok_or_else(|| crate::error::IterError::TagNotFound(n.clone()))
        })
        .collect()
}

#[component]
async fn task_form(task: &Task, tags: &str, #[default] error: Option<String>) -> Result<impl View> {
    let statuses = [TaskStatus::Queue, TaskStatus::Wip, TaskStatus::Done];
    let issue_number = task.github_issue.map(|n| n.to_string()).unwrap_or_default();
    let start = task
        .start_time
        .map(|t| t.format(START_TIME_FMT).to_string())
        .unwrap_or_default();
    let duration = task.duration.map(|d| d.to_string()).unwrap_or_default();
    Ok(view! {
        <h1>"Edit task"</h1>
        error_box(error: &error)
        <form method="post" action=(format!("/task/{}/edit", task.id()))>
            field(name: "name", label: "Name", value: &task.name)
            textarea(text: &task.description)
            <label>"Status"</label>
            <select name="status">
                for s in statuses {
                    <option value=(s.as_str()) if task.status == s { selected="" }>(s.label())</option>
                }
            </select>
            field(name: "github_issue", label: "GitHub issue number", value: &issue_number)
            field(name: "branch_prefix", label: "Branch prefix", value: &task.branch_prefix)
            check(name: "urgency", label: "Urgent", on: task.urgency)
            check(name: "importance", label: "Important", on: task.importance)
            field(name: "start_time", label: "Start (yyyy-mm-dd hh:mm)", value: &start)
            field(name: "duration", label: "Duration (hh:mm)", value: &duration)
            field(name: "tags", label: "Tags (comma-separated)", value: tags)
            form_actions(cancel: format!("/task/{}", task.id()))
        </form>
    })
}

#[page("/task/{id}/edit")]
async fn task_edit(cx: &Cx) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let (nav, task, tags) = {
        let db = open_db()?;
        let task = db.get::<Task>(id)?.ok_or_not_found()?;
        let tags = tag_list(&db.tags_for_task(id)?);
        (Nav::load(&db)?, task, tags)
    };
    Ok(view! { shell(nav: &nav, sel: Sel::Project(task.project_id), task_form(task: &task, tags: &tags)) })
}

#[page(POST "/task/{id}/edit")]
async fn task_save(cx: &Cx, Form(form): Form<TaskForm>) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let (nav, task, error) = {
        let db = open_db()?;
        let existing = db.get::<Task>(id)?.ok_or_not_found()?;
        let (task, valid) = form.apply(existing);
        let saved = valid
            .and_then(|()| tag_ids(&db, &form.tag_names()))
            .and_then(|ids| {
                db.update(id, &task)?;
                db.set_task_tags(id, &ids)
            });
        match saved {
            Ok(()) => return Err(see_other(format!("/task/{id}")).into()),
            Err(e) => (Nav::load(&db)?, task, e.to_string()),
        }
    };
    Ok(view! {
        shell(nav: &nav, sel: Sel::Project(task.project_id), task_form(task: &task, tags: &form.tags, error: Some(error)))
    })
}
