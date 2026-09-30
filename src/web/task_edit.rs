//! The task edit form (`/task/{id}/edit`), the twin of New task
//! (`task_new.rs`): the routes, the loading and the view. The page is
//! `edit_page` and the controls are `task_fields`, the same ones as New
//! task; this supplies the crumbs.

use super::Id;
use super::crumbs::edit_trail;
use super::edit::{Loaded, load, submit};
use super::edit_page::edit_page;
use super::forms::{TaskForm, joined};
use super::load::{Parents, parents_of};
use super::task_fields::task_fields;
use super::ui::form_error::FormError;
use super::url::{task_edit_url, task_url};
use crate::db::{Db, Table};
use crate::error::Result as IterResult;
use crate::models::Task;
use topcoat::{
    Result,
    context::Cx,
    router::{RouterBuilder, content::Form, page, path_param},
    view::{View, component, view},
};

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder.page(show).page(save)
}

/// What the form starts out showing for `task`: its own values, and its tag
/// names comma-separated (one query). The twin of `task_new::blank`.
pub fn values(db: &Db, task: &Task) -> IterResult<TaskForm> {
    Ok(TaskForm::of(task, &joined(&db.tags_for_task(task.id())?)))
}

#[page("/task/{id}/edit")]
async fn show(cx: &Cx) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let page: Loaded<Task, _> = load(id, |db, task: &Task| {
        Ok((parents_of(db, task.project_id)?, values(db, task)?))
    })?;
    let (task, (parents, values)) = (page.row, page.extra);
    Ok(view! {
        screen(stored: &task, parents: &parents, values: &values)
    })
}

#[page(POST "/task/{id}/edit")]
async fn save(cx: &Cx, Form(form): Form<TaskForm>) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    // `form` is what was typed; `refused.stored` the stored task.
    let refused = submit(id, &form, |db, task| parents_of(db, task.project_id))?;
    Ok(view! {
        screen(stored: &refused.stored, parents: &refused.extra, values: &form, error: Some(&refused.error))
    })
}

/// `stored` is the task as stored (the breadcrumb and title of the page, not
/// what was typed), `parents` its project and organization; `values` is what
/// the form shows: [`values`] on GET, what was typed after a refused submit.
/// `error` is the refusal, if any. Same arguments as `task_new::screen`.
#[component]
pub async fn screen(
    stored: &Task,
    parents: &Parents,
    values: &TaskForm,
    #[default] error: Option<&FormError>,
) -> Result<impl View> {
    let base = task_url(stored.id());
    let crumbs = edit_trail(&parents.org, Some(&parents.project), &stored.name, &base);
    Ok(view! {
        edit_page(
            kind: "task",
            subject: &stored.name,
            name: &values.name,
            description: &values.description,
            crumbs: &crumbs,
            action: task_edit_url(stored.id()),
            cancel: base,
            error: error,
            task_fields(form: values, error: error)
        )
    })
}
