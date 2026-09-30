//! The new-task form (`/project/{id}/task/new`): the routes, the loading and
//! the view. Nothing here has rules of its own: it is all shared components
//! (`ui/`). The page is `edit_page` (creating); this supplies the crumbs. The write is `edit::create`, the
//! twin of the edit pages' `edit::submit`.

use super::Id;
use super::crumbs::trail;
use super::edit::create;
use super::edit_page::edit_page;
use super::forms::TaskForm;
use super::load::{Parents, org_of, project_of};
use super::open_db;
use super::task_fields::task_fields;
use super::ui::breadcrumb::Crumb;
use super::ui::form_error::FormError;
use super::url::{new_task_url, project_tasks_url};
use crate::config::config;
use crate::db::{Db, Table};
use crate::git;
use crate::models::{Project, Task};
use topcoat::{
    Result,
    context::Cx,
    router::{RouterBuilder, content::Form, page, path_param},
    view::{View, component, view},
};

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder.page(show).page(save)
}

#[page("/project/{id}/task/new")]
async fn show(cx: &Cx) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let db = open_db()?;
    let new = NewTask::load(&db, id)?;
    let parents = new.parents(&db)?;
    let values = new.values();
    Ok(view! {
        screen(stored: &new.template, parents: &parents, values: &values)
    })
}

#[page(POST "/project/{id}/task/new")]
async fn save(cx: &Cx, Form(form): Form<TaskForm>) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    // Only a refused submission gets here; what was typed is `form`.
    let refused = create(
        &form,
        |db| {
            let new = NewTask::load(db, id)?;
            Ok((new.template.clone(), new))
        },
        |db, new| new.parents(db),
    )?;
    Ok(view! {
        screen(stored: &refused.stored, parents: &refused.extra, values: &form, error: Some(&refused.error))
    })
}

/// What the New task form of a project starts from: the blank task new tasks
/// in the project start as, and the project itself.
struct NewTask {
    /// The template: the project's branch prefix and the configured status.
    template: Task,
    project: Project,
}

impl NewTask {
    /// Loads project `id`. A 404 for no such project.
    fn load(db: &Db, id: i64) -> Result<NewTask> {
        let project = project_of(db, id)?;
        let template = Task::template(
            project.id(),
            git::branch_prefix(&project.branch_template),
            config().task.status,
        );
        Ok(NewTask { template, project })
    }

    /// The first values of the form: the template, no tags. The twin of
    /// `task_edit::values`.
    fn values(&self) -> TaskForm {
        TaskForm::of(&self.template, "")
    }

    /// The project and its organization (one more lookup, only when it has
    /// one).
    fn parents(&self, db: &Db) -> Result<Parents> {
        let org = org_of(db, &self.project)?;
        Ok(Parents {
            project: self.project.clone(),
            org,
        })
    }
}

/// `stored` is the template, `parents` the project the task goes in and its
/// organization; `values` is what the form shows: `blank` on GET, what was
/// typed after a refused submit. `error` is the refusal, if any. Same
/// arguments as `task_edit::screen`.
#[component]
pub async fn screen(
    stored: &Task,
    parents: &Parents,
    values: &TaskForm,
    #[default] error: Option<&FormError>,
) -> Result<impl View> {
    let project = &parents.project;
    let crumbs = trail(&parents.org, Some(project), Crumb::here("New task"));
    Ok(view! {
        edit_page(
            kind: "task",
            subject: &project.name,
            name: &values.name,
            description: &values.description,
            crumbs: &crumbs,
            action: new_task_url(stored.project_id),
            cancel: project_tasks_url(stored.project_id),
            error: error,
            creating: true,
            submit_label: "Create task",
            task_fields(form: values, error: error)
        )
    })
}
