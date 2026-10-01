//! New task: the pop-up over a Tasks tab (`?new=task`, see `ui/modal.rs`)
//! that holds the whole task form, and its write, `POST /task/new`. The
//! first row of the task table opens it. Done, the browser lands back on the
//! list it came from; refused, that page comes back with the pop-up open, the
//! error and what was typed. The write is `edit::create_to`, the twin of the
//! edit pages' `edit::submit`.

use super::Id;
use super::edit::create_to;
use super::forms::{DESCRIPTION, NAME, TaskForm};
use super::load::project_of;
use super::org::{load as load_org, screen as org_screen};
use super::project::{load as load_project, screen as project_screen};
use super::sections::Section;
use super::task_fields::task_fields;
use super::task_query::DEFAULT;
use super::ui::field::{field, select, textarea};
use super::ui::form::{form, required_note};
use super::ui::form_actions::form_actions;
use super::ui::form_error::FormError;
use super::ui::modal::modal;
use super::ui::notice::error_box;
use super::url::{NEW_TASK, org_url, page_url, project_url, tasks_list_url};
use super::{now, open_db};
use crate::config::config;
use crate::db::Table;
use crate::git;
use crate::models::{Project, Task};
use serde::Deserialize;
use topcoat::{
    Result,
    context::Cx,
    router::{
        RouterBuilder,
        content::Form,
        error::{bad_request, see_other},
        page, path_param,
    },
    view::{View, component, view},
};

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder.page(old_form).page(save)
}

/// The Tasks tab a New task pop-up sits on: a project's, or an
/// organization's (the pop-up then asks for the project).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TaskHome {
    Project(i64),
    Org(i64),
}

impl TaskHome {
    fn base(self) -> String {
        match self {
            TaskHome::Project(id) => project_url(id),
            TaskHome::Org(id) => org_url(id),
        }
    }

    /// The list with the search `q`: where a created task lands, and where
    /// the pop-up closes to.
    pub fn list_url(self, q: Option<&str>) -> String {
        tasks_list_url(&self.base(), q)
    }

    /// The list with the pop-up open: where the table's first row leads.
    pub fn open_url(self, q: Option<&str>) -> String {
        page_url(&self.list_url(q), &[("new", Some("task"))])
    }
}

/// The task a new task in `project` starts as: the project's branch prefix
/// and the configured status.
fn template(project: &Project) -> Task {
    Task::template(
        project.id(),
        git::branch_prefix(&project.branch_template),
        config().task.status,
    )
}

/// What the pop-up shows: where it sits (and the list's search `q`, kept
/// through the round trip), the projects to choose from (an organization's
/// list; none on a project's), the chosen project, the form's values and
/// the refusal of the last try, if any.
pub struct TaskCreate {
    pub home: TaskHome,
    pub q: Option<String>,
    /// `(id, name)`; empty when `home` is a project.
    pub projects: Vec<(String, String)>,
    pub project: String,
    pub values: TaskForm,
    pub error: Option<FormError>,
}

impl TaskCreate {
    /// The pop-up of project `project`'s list, as it starts out: the
    /// project's template.
    pub fn in_project(project: &Project, q: Option<&str>) -> TaskCreate {
        TaskCreate {
            home: TaskHome::Project(project.id()),
            q: q.map(String::from),
            projects: Vec::new(),
            project: project.id().to_string(),
            values: TaskForm::of(&template(project), ""),
            error: None,
        }
    }

    /// The pop-up of organization `org`'s list, asking for one of
    /// `projects` (the first chosen); none to choose from, no pop-up. The
    /// branch prefix starts empty: it then follows the chosen project.
    pub fn in_org(org: i64, projects: &[Project], q: Option<&str>) -> Option<TaskCreate> {
        let first = projects.first()?;
        let mut values = TaskForm::of(&template(first), "");
        values.branch_prefix.clear();
        Some(TaskCreate {
            home: TaskHome::Org(org),
            q: q.map(String::from),
            projects: projects
                .iter()
                .map(|p| (p.id().to_string(), p.name.clone()))
                .collect(),
            project: first.id().to_string(),
            values,
            error: None,
        })
    }

    fn close(&self) -> String {
        self.home.list_url(self.q.as_deref())
    }
}

/// The pop-up: the project (an organization's list), Name, Description and
/// the rest of the task's fields, the way out back to the list.
#[component]
pub async fn task_modal(create: &TaskCreate) -> Result<impl View> {
    let error = create.error.as_ref();
    let values = &create.values;
    let org = match create.home {
        TaskHome::Org(id) => Some(id.to_string()),
        TaskHome::Project(_) => None,
    };
    Ok(view! {
        modal(id: "new-task-title", title: "New task", close: create.close(),
            form(
                action: NEW_TASK,
                error_box(error: error)
                required_note()
                if let Some(org) = &org {
                    <input type="hidden" name="org" value=(org.as_str())>
                }
                if let Some(q) = &create.q {
                    <input type="hidden" name="q" value=(q.as_str())>
                }
                if create.projects.is_empty() {
                    <input type="hidden" name=(NewTask::PROJECT) value=(create.project.as_str())>
                } else {
                    select(name: NewTask::PROJECT, label: "Project", options: &create.projects, current: &create.project, required: true, error: error)
                }
                field(name: NAME, label: "Name", value: &values.name, required: true, identifier: true, error: error, autofocus: error.is_none())
                textarea(name: DESCRIPTION, label: "Description", value: &values.description)
                task_fields(form: values, error: error)
                form_actions(cancel: create.close(), submit_label: "Create task")
            )
        )
    })
}

/// What the pop-up posts: the task's form, the project, and where the list
/// is (`org`: an organization's, else the project's own) with its search
/// (`q`), so the browser lands back on it.
#[derive(Deserialize)]
struct NewTask {
    project: String,
    org: Option<String>,
    q: Option<String>,
    #[serde(flatten)]
    form: TaskForm,
}

impl NewTask {
    const PROJECT: &'static str = "project";

    /// The list the task was added from, built from ids (never from text
    /// the request carries, besides the search).
    fn home(&self, project: i64) -> TaskHome {
        match self.org.as_deref().and_then(|o| o.trim().parse().ok()) {
            Some(org) => TaskHome::Org(org),
            None => TaskHome::Project(project),
        }
    }
}

/// The old full-page form: the pop-up on the project's list now.
#[page("/project/{id}/task/new")]
async fn old_form(cx: &Cx) -> Result<()> {
    let id = *path_param::<Id>(cx)?;
    Err(see_other(TaskHome::Project(id).open_url(Some(DEFAULT))).into())
}

/// `POST /task/new`: the task is built from the project's template and the
/// form, as the edit form builds one (the same validation).
#[page(POST "/task/new")]
async fn save(Form(new): Form<NewTask>) -> Result<impl View> {
    let id: i64 = new
        .project
        .trim()
        .parse()
        .map_err(|_| bad_request("Unknown project."))?;
    let home = new.home(id);
    let NewTask {
        project: chosen,
        q,
        form: typed,
        ..
    } = new;
    let db = open_db()?;
    let project = project_of(&db, id)?;
    let start = template(&project);
    let refused = create_to(
        &typed,
        |_| Ok((start, ())),
        |_, ()| Ok(()),
        |_| home.list_url(q.as_deref()),
    )?;
    // Refused: the list again, with the pop-up showing what was typed.
    let now = now();
    let (org_page, project_page) = match home {
        TaskHome::Org(org) => (
            Some(load_org(
                &db,
                org,
                Section::Tasks,
                None,
                None,
                q.as_deref(),
                now,
            )?),
            None,
        ),
        TaskHome::Project(_) => (
            None,
            Some(load_project(&db, id, Section::Tasks, q.as_deref())?),
        ),
    };
    let blank = match &org_page {
        Some(page) => TaskCreate::in_org(page.org.id(), &page.projects, q.as_deref()),
        None => Some(TaskCreate::in_project(&project, q.as_deref())),
    }
    .ok_or_else(|| bad_request("Unknown project."))?;
    let create = TaskCreate {
        project: chosen,
        values: typed,
        error: Some(refused.error),
        ..blank
    };
    Ok(view! {
        if let Some(page) = &org_page {
            org_screen(page: page, section: Section::Tasks, today: now.date(), q: q.as_deref(), task_create: Some(&create))
        } else if let Some(page) = &project_page {
            project_screen(project: &page.parents.project, org: &page.parents.org, tasks: &page.tasks, task_count: page.task_count, section: Section::Tasks, q: q.as_deref(), task_create: Some(&create))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new(org: Option<&str>) -> NewTask {
        let template = Task::template(4, "feat/".into(), crate::models::TaskStatus::Queue);
        NewTask {
            project: "4".into(),
            org: org.map(String::from),
            q: None,
            form: TaskForm::of(&template, ""),
        }
    }

    #[test]
    fn a_new_task_lands_on_the_list_it_came_from() {
        assert_eq!(new(None).home(4).list_url(None), "/project/4?tab=tasks");
        assert_eq!(new(Some("3")).home(4).list_url(None), "/org/3?tab=tasks");
        assert_eq!(
            TaskHome::Org(3).list_url(Some("is:done")),
            "/org/3?tab=tasks&q=is%3Adone"
        );
        // an org that is not a number is not followed
        assert_eq!(new(Some("x/../")).home(4), TaskHome::Project(4));
    }

    #[test]
    fn the_first_row_opens_the_pop_up_on_the_same_list() {
        assert_eq!(
            TaskHome::Project(4).open_url(None),
            "/project/4?tab=tasks&new=task"
        );
        assert_eq!(
            TaskHome::Org(3).open_url(Some("tag:x")),
            "/org/3?tab=tasks&q=tag%3Ax&new=task"
        );
    }

    /// An organization's pop-up asks for the project, starting on the first;
    /// with no projects there is none.
    #[test]
    fn an_organization_pop_up_chooses_among_its_projects() {
        assert!(TaskCreate::in_org(3, &[], None).is_none());
        let project = |id, name: &str| Project {
            id: Some(id),
            name: name.into(),
            branch_template: "feat/{task}".into(),
            ..Project::default()
        };
        let create =
            TaskCreate::in_org(3, &[project(9, "app"), project(5, "cli")], None).expect("pop-up");
        assert_eq!(create.project, "9");
        assert_eq!(create.projects.len(), 2);
        assert!(create.values.branch_prefix.is_empty());
        assert_eq!(create.close(), "/org/3?tab=tasks");
    }
}
