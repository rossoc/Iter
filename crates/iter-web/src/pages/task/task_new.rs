//! New task: the pop-up over a Tasks tab (`?new=task`, `ui/modal.rs`) and its write, `POST /task/new`.

use crate::forms::TaskForm;
use crate::forms::edit::create_to;
use crate::load::project_of;
use crate::pages::org::org::load as load_org;
use crate::pages::org::org::screen as org_screen;
use crate::pages::project::project::load as load_project;
use crate::pages::project::project::screen as project_screen;
use crate::parts::all::*;
use crate::prelude::*;
use iter_core::db::Table;
use iter_core::front::query::DEFAULT;
use serde::Deserialize;

#[derive(Deserialize)]
pub(super) struct NewTask {
    project: String,
    org: Option<String>,
    q: Option<String>,
    #[serde(flatten)]
    form: TaskForm,
}

impl NewTask {
    /// Built from ids, never from text the request carries (besides the search).
    fn home(&self, project: i64) -> TaskHome {
        match self.org.as_deref().and_then(|o| o.trim().parse().ok()) {
            Some(org) => TaskHome::Org(org),
            None => TaskHome::Project(project),
        }
    }
}

#[page("/project/{id}/task/new")]
pub(super) async fn old_form(cx: &Cx) -> Result<()> {
    let id = path_id(cx)?;
    Err(see_other(TaskHome::Project(id).open_url(Some(DEFAULT))).into())
}

#[page(POST "/task/new")]
pub(super) async fn save(Form(new): Form<NewTask>) -> Result<impl View> {
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
    let db = fresh_db()?;
    let project = project_of(&db, id)?;
    let start = template(&project);
    let refused = create_to(
        &typed,
        |_| Ok((start, ())),
        |_, ()| Ok(()),
        |_| home.list_url(q.as_deref()),
    )?;
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
    use iter_core::models::Task;

    fn new(org: Option<&str>) -> NewTask {
        let template = Task::template(4, "feat/".into(), iter_core::models::TaskStatus::Queue);
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
}
