//! The project edit form (`/project/{id}/edit`), the sibling of the
//! organization one: the routes and the view. The page is `edit_page`; this
//! supplies the crumbs and the controls of its own.

use super::Id;
use super::crumbs::edit_trail;
use super::edit::{Loaded, load, submit};
use super::edit_page::edit_page;
use super::forms::ProjectForm;
use super::load::org_in;
use super::settings_fields::settings_fields;
use super::ui::field::{field, id_text, optional_options, select};
use super::ui::form_error::FormError;
use super::url::{project_edit_url, project_url};
use crate::db::Table;
use crate::models::{Configured, Organization, Project};
use topcoat::{
    Result,
    context::Cx,
    router::{RouterBuilder, content::Form, page, path_param},
    view::{View, component, view},
};

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder.page(show).page(save)
}

#[page("/project/{id}/edit")]
async fn show(cx: &Cx) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let page: Loaded<Project, _> = load(id, |db, _| Ok(db.list::<Organization>()?))?;
    let (project, orgs) = (page.row, page.extra);
    let org = org_in(&orgs, &project).cloned();
    Ok(view! {
        screen(project: &project, stored: &project, org: &org, orgs: &orgs)
    })
}

#[page(POST "/project/{id}/edit")]
async fn save(cx: &Cx, Form(form): Form<ProjectForm>) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    // `refused.row` holds what was typed, `refused.stored` the stored row.
    let refused = submit(id, &form, |db, _| Ok(db.list::<Organization>()?))?;
    let org = org_in(&refused.extra, &refused.stored).cloned();
    Ok(view! {
        screen(project: &refused.row, stored: &refused.stored, org: &org, orgs: &refused.extra, error: Some(&refused.error))
    })
}

/// `stored` is the project as stored (the breadcrumb and title of the page,
/// not what was typed) and `org` its organization. `project` holds what the
/// form shows, and `orgs` are the choices. `error` is the refusal, if any.
#[component]
pub async fn screen(
    project: &Project,
    stored: &Project,
    org: &Option<Organization>,
    orgs: &[Organization],
    #[default] error: Option<&FormError>,
) -> Result<impl View> {
    let base = project_url(stored.id());
    let crumbs = edit_trail(org, None, &stored.name, &base);
    let options = optional_options(
        "No organization",
        orgs.iter().map(|o| (o.id(), o.name.as_str())),
    );
    let current = id_text(project.organization_id);
    let settings = project.settings();
    Ok(view! {
        edit_page(
            kind: "project",
            subject: &stored.name,
            name: &project.name,
            description: &project.description,
            crumbs: &crumbs,
            action: project_edit_url(stored.id()),
            cancel: base,
            error: error,
            field(
                name: ProjectForm::BASE_PATH,
                label: "Base path",
                value: &project.base_path,
                hint: "The project's directory on disk. ~ is expanded and a relative path is made absolute.",
                required: true,
                identifier: true,
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
