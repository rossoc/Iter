//! New project: the pop-up over a project list (`?new=project`, see
//! `ui/modal.rs`) with the whole project form, and its write (`POST
//! /{org|board}/{id}/project/quick`). The list's first row opens it. An
//! organization's list makes a member of it, a board's a project outside any
//! organization that sits on the board. Only the row is written (as editing
//! does: no directory is made), from the template `iter init` starts from --
//! the config's project defaults, the organization's settings over them --
//! and with the edit form's validation. The base path is typed or chosen
//! with Choose: the system's folder picker (`folder_picker.rs`), or, where
//! there is none, the folder browser (`folders.rs`); both come back to the
//! pop-up.

use super::board_info::{load as load_board, screen as board_screen};
use super::edit::create_to;
use super::folder_picker::{Picked, pick};
use super::folders::start_folder;
use super::forms::{DESCRIPTION, NAME, ProjectForm, SettingsForm};
use super::org::{load as load_org, screen as org_screen};
use super::sections::Section;
use super::settings_fields::settings_fields;
use super::ui::field::{field, textarea};
use super::ui::form::{form, required_note};
use super::ui::form_actions::form_actions;
use super::ui::form_error::FormError;
use super::ui::modal::modal;
use super::ui::notice::error_box;
use super::ui::{FOLDER, icon};
use super::url::{
    PICK_FOLDER, board_info_url, folders_url, org_url, page_url, quick_board_project_url,
    quick_project_url,
};
use super::{Id, now, open_db};
use crate::config::{config, expand_home};
use crate::models::{Board, Configured, Organization, Project, Settings};
use serde::Deserialize;
use topcoat::{
    Result,
    context::Cx,
    router::{
        RouterBuilder,
        content::Form,
        error::{RouterErrorExt, bad_request, see_other},
        page, path_param,
    },
    view::{Child, View, component, view},
};

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder.page(quick_org).page(quick_board).page(pick_folder)
}

/// The list a project is added to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Home {
    Org(i64),
    Board(i64),
}

impl Home {
    /// The token the pop-up and the folder browser carry (`org:3`).
    pub fn token(self) -> String {
        match self {
            Home::Org(id) => format!("org:{id}"),
            Home::Board(id) => format!("board:{id}"),
        }
    }

    pub fn parse(token: &str) -> Option<Home> {
        let (kind, id) = token.trim().split_once(':')?;
        let id = id.parse().ok()?;
        match kind {
            "org" => Some(Home::Org(id)),
            "board" => Some(Home::Board(id)),
            _ => None,
        }
    }

    /// Where the pop-up posts.
    pub fn quick_url(self) -> String {
        match self {
            Home::Org(id) => quick_project_url(id),
            Home::Board(id) => quick_board_project_url(id),
        }
    }

    /// The page with the list, where a created project lands.
    pub fn page_url(self) -> String {
        match self {
            Home::Org(id) => org_url(id),
            Home::Board(id) => board_info_url(id),
        }
    }

    /// The page with the pop-up open: where the list's first row leads.
    pub fn open_url(self) -> String {
        self.open_url_with("", "")
    }

    /// [`Self::open_url`] with the name and base path to start with (what
    /// the folder browser sends back); empty ones are left out.
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

    /// The pop-up as it starts out: `name` and `base_path` (from the folder
    /// browser, else empty), the template's settings, no error. A 404 when
    /// the list is not there.
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

    /// The project to start from, and a 404 when the list is not there.
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

    /// The list must exist (a 404 otherwise): the folder browser checks it.
    pub fn exists(self) -> Result<()> {
        self.template().map(|_| ())
    }
}

/// What the New project pop-up shows: the list it adds to, what is typed
/// (or starts out), and the refusal of the last try, if any.
pub struct ProjectCreate {
    pub home: Home,
    pub name: String,
    pub description: String,
    pub base_path: String,
    pub settings: Settings,
    pub error: Option<FormError>,
}

/// The pop-up: Name, Description, Base path (typed, or with Choose beside
/// it), the Settings group, the way out back to the list.
///
/// Enter in a field submits with the form's first submit button, so the
/// first one is a hidden Create project: Choose (which posts the form to the
/// folder picker, [`pick_folder`]) must not be the default.
#[component]
pub async fn project_modal(create: &ProjectCreate) -> Result<impl View> {
    let error = create.error.as_ref();
    let close = create.home.page_url();
    Ok(view! {
        modal(id: "new-project-title", title: "New project", close: close.as_str(),
            form(
                action: create.home.quick_url(),
                <button class="sr" type="submit" tabindex="-1" aria-hidden="true">"Create project"</button>
                error_box(error: error)
                required_note()
                <input type="hidden" name="home" value=(create.home.token())>
                field(name: NAME, label: "Name", value: &create.name, required: true, identifier: true, error: error, autofocus: error.is_none())
                textarea(name: DESCRIPTION, label: "Description", value: &create.description)
                field(name: ProjectForm::BASE_PATH, label: "Base path", value: &create.base_path, required: true, identifier: true, error: error,
                    hint: "The project's directory on disk, such as ~/src/app. ~ is expanded and a relative path is made absolute. Choose opens the folder picker there, or in your home folder.",
                    beside: Some(Child::new(view! {
                        <button class="button" type="submit" formaction=(PICK_FOLDER)>(icon(FOLDER)) "Choose\u{2026}"</button>
                    })))
                settings_fields(s: &create.settings)
                form_actions(cancel: close.as_str(), submit_label: "Create project")
            )
        )
    })
}

/// What the pop-up posts (it also posts `home`, which the route's path
/// already says).
#[derive(Deserialize)]
pub struct NewProject {
    pub name: String,
    pub description: String,
    pub base_path: String,
    #[serde(flatten)]
    pub settings: SettingsForm,
}

/// Makes the project. Done, the browser goes to the list's page (this does
/// not return); refused, the pop-up to show again: the error and what was
/// typed.
fn add(home: Home, typed: NewProject) -> Result<ProjectCreate> {
    let template = home.template()?;
    let project = ProjectForm {
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

/// The page of the list `home`, with the pop-up showing `create`.
#[component]
async fn list_page(home: Home, create: &ProjectCreate) -> Result<impl View> {
    let db = open_db()?;
    let now = now();
    let (org_page, board_page) = match home {
        Home::Org(id) => (
            Some(load_org(&db, id, Section::Info, None, None, None, now)?),
            None,
        ),
        Home::Board(id) => (None, Some(load_board(&db, id)?)),
    };
    Ok(view! {
        if let Some(page) = &org_page {
            org_screen(page: page, section: Section::Info, today: now.date(), project_create: Some(create))
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

/// What Choose posts: the whole pop-up, so nothing typed is lost.
#[derive(Deserialize)]
struct PickFolder {
    home: String,
    #[serde(flatten)]
    typed: NewProject,
}

/// `POST /folders/pick`: opens the system's folder picker at the typed base
/// path (the nearest folder that exists; else the home folder) and shows the
/// pop-up again with the chosen folder as the base path, and its name as the
/// project's when none was typed. Cancelled, the pop-up comes back as it
/// was. With no picker on the system, the folder browser instead.
#[page(POST "/folders/pick")]
async fn pick_folder(Form(posted): Form<PickFolder>) -> Result<impl View> {
    let home = Home::parse(&posted.home).ok_or_else(|| bad_request("Unknown list."))?;
    home.exists()?;
    let NewProject {
        mut name,
        description,
        mut base_path,
        settings,
    } = posted.typed;
    let start = start_folder(&base_path, &expand_home("~"));
    match pick(&start) {
        Picked::Folder(folder) => {
            if name.trim().is_empty() {
                name = folder
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
            }
            base_path = folder.to_string_lossy().into_owned();
        }
        Picked::Cancelled => {}
        Picked::Unavailable => {
            let browse = folders_url(&home.token(), &name, &start.to_string_lossy(), false);
            return Err(see_other(browse).into());
        }
    }
    let create = ProjectCreate {
        home,
        name,
        description,
        base_path,
        settings: settings.settings(),
        error: None,
    };
    Ok(view! { list_page(home: home, create: &create) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::edit::EditForm;

    #[test]
    fn a_home_round_trips_through_its_token() {
        for home in [Home::Org(3), Home::Board(7)] {
            assert_eq!(Home::parse(&home.token()), Some(home));
        }
        assert_eq!(Home::parse("org:x"), None);
        assert_eq!(Home::parse("project:3"), None);
        assert_eq!(Home::parse("org"), None);
        assert_eq!(Home::Org(3).quick_url(), "/org/3/project/quick");
        assert_eq!(Home::Board(7).quick_url(), "/board/7/project/quick");
        assert_eq!(Home::Board(7).page_url(), "/board/7/info");
        assert_eq!(Home::Org(3).open_url(), "/org/3?new=project");
        assert_eq!(
            Home::Board(7).open_url_with("my app", "/src"),
            "/board/7/info?new=project&name=my%20app&base_path=%2Fsrc"
        );
    }

    /// The settings of a form that keep `settings` as they are.
    fn settings_form(settings: &Settings) -> SettingsForm {
        let on = |set: bool| set.then(|| "on".to_string());
        SettingsForm {
            github: on(settings.github),
            tmux: on(settings.tmux),
            auto_branch: on(settings.auto_branch),
            branch_template: settings.branch_template.clone(),
            default_branch: settings.default_branch.clone(),
            github_project: settings.github_project.clone(),
        }
    }

    /// The settings a quick project is made with survive the form: the
    /// organization's, over the config's.
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
            organization: String::new(),
            settings: settings_form(&template.settings()),
        };
        let (project, valid) = typed.apply(template);
        assert!(valid.is_ok());
        assert_eq!((project.organization_id, project.board_id), (None, Some(7)));
    }
}
