//! New project: the pop-up over a project list (`?new=project`, `ui/modal.rs`) and its write (`POST /{org|board}/{id}/project/quick`).

use crate::{
    forms::{ProjectForm, SettingsForm, edit::create_to},
    pages::{
        board::board_info::{load as load_board, screen as board_screen},
        org::org::{load as load_org, screen as org_screen},
    },
    parts::all::*,
    prelude::*,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub(crate) struct NewProject {
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
    let db = fresh_db()?;
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
pub(super) async fn quick_org(cx: &Cx, Form(typed): Form<NewProject>) -> Result<impl View> {
    let id = path_id(cx)?;
    let create = add(Home::Org(id), typed)?;
    Ok(view! { list_page(home: Home::Org(id), create: &create) })
}

#[page(POST "/board/{id}/project/quick")]
pub(super) async fn quick_board(cx: &Cx, Form(typed): Form<NewProject>) -> Result<impl View> {
    let id = path_id(cx)?;
    let create = add(Home::Board(id), typed)?;
    Ok(view! { list_page(home: Home::Board(id), create: &create) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forms::edit::EditForm;
    use iter_core::models::{Configured, Organization, Project, Settings};

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
