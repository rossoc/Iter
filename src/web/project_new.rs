//! New project: the pop-up over a project list (`?new=project`, `ui/modal.rs`) and its write (`POST /{org|board}/{id}/project/quick`).

use super::board_info::{load as load_board, screen as board_screen};
use super::edit::create_to;
use super::forms::{DESCRIPTION, NAME, ProjectForm, SettingsForm};
use super::org::{load as load_org, screen as org_screen};
use super::sections::Section;
use super::settings_fields::settings_fields;
use super::ui::field::{field, path_field, textarea};
use super::ui::form::form_shell;
use super::ui::form_error::FormError;
use super::ui::modal::modal;
use super::url::{
    board_info_url, org_url, page_url, projects_tab_url, quick_board_project_url, quick_project_url,
};
use super::{Id, now, open_db};
use crate::config::config;
use crate::models::{Board, Configured, Organization, Project, Settings};
use serde::Deserialize;
use topcoat::{
    Result,
    context::Cx,
    router::{RouterBuilder, content::Form, error::RouterErrorExt, page, path_param},
    view::{View, component, view},
};

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder.page(quick_org).page(quick_board)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Home {
    Org(i64),
    Board(i64),
}

impl Home {
    pub fn quick_url(self) -> String {
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

    pub fn open_url_with(self, name: &str, base_path: &str) -> String {
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

    fn template(self) -> Result<Project> {
        let db = open_db()?;
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

#[derive(Deserialize)]
pub struct NewProject {
    pub name: String,
    pub description: String,
    pub base_path: String,
    #[serde(flatten)]
    pub settings: SettingsForm,
}

/// Done, the browser goes to the list's page (this does not return); refused, the pop-up shows again.
fn add(home: Home, typed: NewProject) -> Result<ProjectCreate> {
    let template = home.template()?;
    let project = ProjectForm {
        notes_path: String::new(),
        name: typed.name,
        description: typed.description,
        base_path: typed.base_path,
        organization: match home {
            Home::Org(id) => id.to_string(),
            Home::Board(_) => String::new(),
        },
        settings: typed.settings,
    };
    let refused = create_to(
        &project,
        |_| Ok((template.clone(), ())),
        |_, ()| Ok(()),
        |_| home.page_url(),
    )?;
    Ok(ProjectCreate {
        home,
        settings: project.settings.settings(),
        name: project.name,
        description: project.description,
        base_path: project.base_path,
        error: Some(refused.error),
    })
}

#[component]
async fn list_page(home: Home, create: &ProjectCreate) -> Result<impl View> {
    let db = open_db()?;
    let now = now();
    let (org_page, board_page) = match home {
        Home::Org(id) => (
            Some(load_org(&db, id, Section::Projects, None, None, None, now)?),
            None,
        ),
        Home::Board(id) => (None, Some(load_board(&db, id)?)),
    };
    Ok(view! {
        if let Some(page) = &org_page {
            org_screen(page: page, section: Section::Projects, today: now.date(), project_create: Some(create))
        } else if let Some((board, projects)) = &board_page {
            board_screen(board: board, projects: projects, project_create: Some(create))
        }
    })
}

#[page(POST "/org/{id}/project/quick")]
async fn quick_org(cx: &Cx, Form(typed): Form<NewProject>) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let create = add(Home::Org(id), typed)?;
    Ok(view! { list_page(home: Home::Org(id), create: &create) })
}

#[page(POST "/board/{id}/project/quick")]
async fn quick_board(cx: &Cx, Form(typed): Form<NewProject>) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let create = add(Home::Board(id), typed)?;
    Ok(view! { list_page(home: Home::Board(id), create: &create) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::edit::EditForm;

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

    fn settings_form(settings: &Settings) -> SettingsForm {
        SettingsForm {
            github: settings.github,
            tmux: settings.tmux,
            auto_branch: settings.auto_branch,
            branch_template: settings.branch_template.clone(),
            default_branch: settings.default_branch.clone(),
            github_project: settings.github_project.clone(),
        }
    }

    /// The settings a quick project is made with survive the form: the organization's, over the config's.
    #[test]
    fn a_quick_project_keeps_the_organization_settings() {
        let org = Organization {
            id: Some(3),
            name: "acme".into(),
            github: true,
            tmux: false,
            branch_template: "acme/".into(),
            default_branch: "trunk".into(),
            ..Organization::default()
        };
        let mut template = Project::template(&Settings::default());
        template.inherit_from(&org);
        let typed = ProjectForm {
            name: "p".into(),
            description: String::new(),
            base_path: "/tmp/p".into(),
            notes_path: String::new(),
            organization: "3".into(),
            settings: settings_form(&template.settings()),
        };
        let (project, valid) = typed.apply(template);
        assert!(valid.is_ok());
        assert_eq!(project.organization_id, Some(3));
        assert!(project.github && !project.tmux);
        assert_eq!(project.branch_template, "acme/");
        assert_eq!(project.default_branch, "trunk");
        let (_, blank) = ProjectForm {
            base_path: " ".into(),
            ..typed
        }
        .apply(Project::default());
        assert!(blank.is_err());
    }

    /// A board's project belongs to no organization, and keeps its board.
    #[test]
    fn a_board_project_has_no_organization_and_keeps_its_board() {
        let mut template = Project::template(&Settings::default());
        template.board_id = Some(7);
        let typed = ProjectForm {
            name: "p".into(),
            description: String::new(),
            base_path: "/tmp/p".into(),
            notes_path: String::new(),
            organization: String::new(),
            settings: settings_form(&template.settings()),
        };
        let (project, valid) = typed.apply(template);
        assert!(valid.is_ok());
        assert_eq!((project.organization_id, project.board_id), (None, Some(7)));
    }
}
