//! The project edit form (`/project/{id}/edit`); the page is `edit_page`.

use crate::{
    forms::{
        ProjectForm,
        edit::{Loaded, load, submit},
    },
    load::org_in,
    parts::all::*,
    prelude::*,
};
use iter_core::{
    db::Table,
    models::{Configured, Organization, Project},
};

#[page("/project/{id}/edit")]
pub(super) async fn show(cx: &Cx) -> Result<impl View> {
    let id = path_id(cx)?;
    let page: Loaded<Project, _> = load(id, |db, _| Ok(db.list::<Organization>()?))?;
    let (project, orgs) = (page.row, page.extra);
    let org = org_in(&orgs, &project).cloned();
    Ok(view! {
        screen(project: &project, stored: &project, org: &org, orgs: &orgs)
    })
}

#[page(POST "/project/{id}/edit")]
pub(super) async fn save(cx: &Cx, Form(form): Form<ProjectForm>) -> Result<impl View> {
    let id = path_id(cx)?;
    let refused = submit(id, &form, |db, _| Ok(db.list::<Organization>()?))?;
    let org = org_in(&refused.extra, &refused.stored).cloned();
    Ok(view! {
        screen(project: &refused.row, stored: &refused.stored, org: &org, orgs: &refused.extra, error: Some(&refused.error))
    })
}

#[component]
pub async fn screen(
    project: &Project,
    stored: &Project,
    org: &Option<Organization>,
    orgs: &[Organization],
    #[default] error: Option<&FormError>,
) -> Result<impl View> {
    let base = project_url(stored.id());
    let crumbs = trail(org, None, Crumb::label("Project"));
    let options = optional_options(
        "No organization",
        orgs.iter().map(|o| (o.id(), o.name.as_str())),
    );
    let current = id_text(project.organization_id);
    let settings = project.settings();
    let tasks_n = fresh_db()?.count_tasks_for_project(stored.id())?;
    let notes = NotesTab::load(iter_core::notes::project_dir(stored, org), false);
    let sections = section_tabs(
        &base,
        Section::Settings,
        Counts {
            tasks: tasks_n,
            notes: NotesTab::badge(Some(&notes)),
            wiki: true,
            ..Counts::default()
        },
    );
    Ok(view! {
        edit_page(
            kind: "project",
            subject: &stored.name,
            name: &project.name,
            description: &project.description,
            crumbs: &crumbs,
            sections: Some(&sections),
            action: project_edit_url(stored.id()),
            cancel: base,
            error: error,
            path_field(
                name: ProjectForm::BASE_PATH,
                label: "Base path",
                value: &project.base_path,
                hint: "The project's directory on disk. ~ is expanded and a relative path is made absolute.",
                required: true,
                error: error
            )
            path_field(
                name: ProjectForm::NOTES_PATH,
                label: "Notes path",
                value: &project.notes_path,
                hint: "Where the project's daily notes are kept. Left empty, they go in a folder of the project's name inside the organization's notes path.",
                error: error
            )
            select(
                name: ProjectForm::ORGANIZATION,
                label: "Organization",
                options: &options,
                current: &current,
                hint: "The organization the project belongs to. Choose No organization for none.",
                error: error
            )
            settings_fields(s: &settings)
        )
    })
}
