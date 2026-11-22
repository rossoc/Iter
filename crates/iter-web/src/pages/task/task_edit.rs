//! The task edit form (`/task/{id}/edit`), the twin of New task (`task_new.rs`); the controls are in `task_fields`.

use crate::{
    forms::{
        TaskForm,
        edit::{Loaded, load, submit},
        joined,
    },
    load::{Parents, parents_of},
    parts::all::*,
    prelude::*,
};
use iter_core::{
    agent::taskdir,
    db::{Db, Table},
    error::Result as IterResult,
    models::{Project, Task},
};

pub fn values(db: &Db, task: &Task) -> IterResult<TaskForm> {
    Ok(TaskForm::of(task, &joined(&db.tags_for_task(task.id())?)))
}

fn project_options(db: &Db) -> IterResult<Vec<(String, String)>> {
    Ok(db
        .list::<Project>()?
        .into_iter()
        .map(|p| (p.id().to_string(), p.name))
        .collect())
}

#[page("/task/{id}/edit")]
pub(super) async fn show(cx: &Cx) -> Result<impl View> {
    let id = path_id(cx)?;
    let page: Loaded<Task, _> = load(id, |db, task: &Task| {
        Ok((
            parents_of(db, task.project_id)?,
            values(db, task)?,
            project_options(db)?,
        ))
    })?;
    let (task, (parents, values, projects)) = (page.row, page.extra);
    Ok(view! {
        screen(stored: &task, parents: &parents, projects: &projects, values: &values)
    })
}

#[page(POST "/task/{id}/edit")]
pub(super) async fn save(cx: &Cx, Form(form): Form<TaskForm>) -> Result<impl View> {
    let id = path_id(cx)?;
    let refused = submit(id, &form, |db, task| {
        Ok((parents_of(db, task.project_id)?, project_options(db)?))
    })?;
    let (parents, projects) = refused.extra;
    Ok(view! {
        screen(stored: &refused.stored, parents: &parents, projects: &projects, values: &form, error: Some(&refused.error))
    })
}

#[component]
pub async fn screen(
    stored: &Task,
    parents: &Parents,
    projects: &[(String, String)],
    values: &TaskForm,
    #[default] error: Option<&FormError>,
) -> Result<impl View> {
    let base = task_url(stored.id());
    let crumbs = trail(&parents.org, Some(&parents.project), Crumb::label("Task"));
    let db = fresh_db()?;
    let sessions_n = db.sessions_for_task(stored.id())?.len();
    let agents_n = taskdir::of(&db, &parents.project, stored)
        .map(|d| d.runs().len())
        .unwrap_or(0);
    let sections = task_tabs(&base, TaskSection::Settings, sessions_n, agents_n);
    Ok(view! {
        edit_page(
            kind: "task",
            subject: &stored.name,
            name: &values.name,
            description: &values.description,
            crumbs: &crumbs,
            sections: Some(&sections),
            action: task_edit_url(stored.id()),
            cancel: base,
            error: error,
            select(name: TaskForm::MOVE_TO, label: "Project", options: projects, current: &values.move_to, required: true, error: error,
                hint: "Moving a task needs its session torn down first.")
            task_fields(form: values, error: error)
        )
    })
}
