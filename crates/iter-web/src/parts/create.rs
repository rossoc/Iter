//! What the "New project" and "New task" pop-ups need: where they open from (`Home`, `TaskHome`),
//! what they show (`ProjectCreate`, `TaskCreate`) and the two views. Pure data and views, so a
//! host page (organization, project, board) can show one without importing the module that
//! handles its POST (`project_new.rs`, `task_new.rs`), which in turn renders the host.

use crate::{forms::{ProjectForm, TaskForm, edit::{DESCRIPTION, NAME}}, parts::all::*, prelude::*};
use iter_core::{config::config, db::Table, git, models::{Board, Configured, Organization, Project, Settings, Task}};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Home {
    Org(i64),
    Board(i64),
}

impl Home {
    pub(crate) fn quick_url(self) -> String {
        match self {
            Home::Org(id) => quick_project_url(id),
            Home::Board(id) => quick_board_project_url(id),
        }
    }

    pub fn page_url(self) -> String {
        match self {
            Home::Org(id) => projects_tab_url(&org_url(id)),
            Home::Board(id) => board_info_url(id),
        }
    }

    pub fn open_url(self) -> String {
        self.open_url_with("", "")
    }

    pub(crate) fn open_url_with(self, name: &str, base_path: &str) -> String {
        let some = |v: &str| (!v.is_empty()).then(|| v.to_string());
        let (name, base_path) = (some(name), some(base_path));
        page_url(
            &self.page_url(),
            &[
                ("new", Some("project")),
                ("name", name.as_deref()),
                ("base_path", base_path.as_deref()),
            ],
        )
    }

    pub fn create(self, name: Option<&str>, base_path: Option<&str>) -> Result<ProjectCreate> {
        let template = self.template()?;
        Ok(ProjectCreate {
            home: self,
            name: name.unwrap_or("").to_string(),
            description: String::new(),
            base_path: base_path.unwrap_or("").to_string(),
            settings: template.settings(),
            error: None,
        })
    }

    pub(crate) fn template(self) -> Result<Project> {
        let db = fresh_db()?;
        let mut template = Project::template(&config().project);
        match self {
            Home::Org(id) => {
                let org = db.get::<Organization>(id)?.ok_or_not_found()?;
                template.inherit_from(&org);
            }
            Home::Board(id) => {
                db.get::<Board>(id)?.ok_or_not_found()?;
                template.board_id = Some(id);
            }
        }
        Ok(template)
    }
}

pub struct ProjectCreate {
    pub home: Home,
    pub name: String,
    pub description: String,
    pub base_path: String,
    pub settings: Settings,
    pub error: Option<FormError>,
}

#[component]
pub async fn project_modal(create: &ProjectCreate) -> Result<impl View> {
    let error = create.error.as_ref();
    let close = create.home.page_url();
    Ok(view! {
        modal(id: "new-project-title", title: "New project", close: close.as_str(),
            form_shell(action: create.home.quick_url(), error: error, cancel: close.as_str(), submit_label: "Create project",
                field(name: NAME, label: "Name", value: &create.name, required: true, identifier: true, error: error, autofocus: error.is_none())
                textarea(name: DESCRIPTION, label: "Description", value: &create.description)
                path_field(name: ProjectForm::BASE_PATH, label: "Base path", value: &create.base_path, required: true, error: error,
                    hint: "The project's directory on disk, such as ~/src/app. ~ is expanded and a relative path is made absolute.")
                settings_fields(s: &create.settings)
            )
        )
    })
}

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

    pub(crate) fn list_url(self, q: Option<&str>) -> String {
        tasks_list_url(&self.base(), q)
    }

    pub fn open_url(self, q: Option<&str>) -> String {
        page_url(&self.list_url(q), &[("new", Some("task"))])
    }
}

pub(crate) fn template(project: &Project) -> Task {
    Task::template(
        project.id(),
        git::branch_prefix(&project.branch_template),
        config().task.status,
    )
}

pub struct TaskCreate {
    pub home: TaskHome,
    pub q: Option<String>,
    pub projects: Vec<(String, String)>,
    pub project: String,
    pub values: TaskForm,
    pub error: Option<FormError>,
}

impl TaskCreate {
    pub(crate) const PROJECT: &'static str = "project";

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

    /// The branch prefix starts empty: it then follows the chosen project.
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
            form_shell(action: NEW_TASK, error: error, cancel: create.close(), submit_label: "Create task",
                if let Some(org) = &org {
                    <input type="hidden" name="org" value=(org.as_str())>
                }
                if let Some(q) = &create.q {
                    <input type="hidden" name="q" value=(q.as_str())>
                }
                if create.projects.is_empty() {
                    <input type="hidden" name=(TaskCreate::PROJECT) value=(create.project.as_str())>
                } else {
                    select(name: TaskCreate::PROJECT, label: "Project", options: &create.projects, current: &create.project, required: true, error: error)
                }
                field(name: NAME, label: "Name", value: &values.name, required: true, identifier: true, error: error, autofocus: error.is_none())
                textarea(name: DESCRIPTION, label: "Description", value: &values.description)
                task_fields(form: values, error: error)
            )
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_home_knows_its_urls() {
        assert_eq!(Home::Org(3).quick_url(), "/org/3/project/quick");
        assert_eq!(Home::Board(7).quick_url(), "/board/7/project/quick");
        assert_eq!(Home::Board(7).page_url(), "/board/7/info");
        assert_eq!(Home::Org(3).open_url(), "/org/3?tab=projects&new=project");
        assert_eq!(
            Home::Board(7).open_url_with("my app", "/src"),
            "/board/7/info?new=project&name=my%20app&base_path=%2Fsrc"
        );
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

    #[test]
    fn an_organization_pop_up_chooses_among_its_projects() {
        iter_core::config::load_defaults(); // `Project::default()` reads the config
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
        assert_eq!(create.close(), "/org/3?tab=tasks")
    }
}
