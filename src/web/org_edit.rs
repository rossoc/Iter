//! The organization edit form (`/org/{id}/edit`): the routes and the view.
//! The page is `edit_page`; this supplies the crumbs and the controls of its
//! own (the Projects checklist is the board form's, built by
//! `project_options`).

use super::Id;
use super::crumbs::edit_trail;
use super::edit::{Loaded, load, submit};
use super::edit_page::edit_page;
use super::forms::OrgForm;
use super::project_options::{group_options, project_checklist};
use super::settings_fields::settings_fields;
use super::ui::field::CheckOption;
use super::ui::form_error::FormError;
use super::url::{org_edit_url, org_url};
use crate::db::Table;
use crate::models::{Configured, Organization};
use topcoat::{
    Result,
    context::Cx,
    router::{RouterBuilder, content::Form, page, path_param},
    view::{View, component, view},
};

pub fn register(builder: RouterBuilder) -> RouterBuilder {
    builder.page(show).page(save)
}

#[page("/org/{id}/edit")]
async fn show(cx: &Cx) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let page: Loaded<Organization, _> =
        load(id, |db, _| Ok(group_options::<Organization>(db, id, None)?))?;
    let (org, projects) = (page.row, page.extra);
    Ok(view! {
        screen(org: &org, stored: &org, projects: &projects)
    })
}

#[page(POST "/org/{id}/edit")]
async fn save(cx: &Cx, Form(pairs): Form<Vec<(String, String)>>) -> Result<impl View> {
    let id = *path_param::<Id>(cx)?;
    let form = OrgForm::parse(&pairs);
    // `refused.row` holds what was typed, `refused.stored` the stored row.
    let refused = submit(id, &form, |db, _| {
        Ok(group_options::<Organization>(db, id, Some(&form.projects))?)
    })?;
    Ok(view! {
        screen(org: &refused.row, stored: &refused.stored, projects: &refused.extra, error: Some(&refused.error))
    })
}

/// `stored` is the organization as stored (the breadcrumb and title of the
/// page, not what was typed). `org` holds what the form shows, and
/// `projects` the checklist's options: the stored roster on GET, what was
/// ticked after a refused submit. `error` is the refusal, if any.
#[component]
pub async fn screen(
    org: &Organization,
    stored: &Organization,
    projects: &[CheckOption],
    #[default] error: Option<&FormError>,
) -> Result<impl View> {
    let base = org_url(stored.id());
    let crumbs = edit_trail(&None, None, &stored.name, &base);
    let settings = org.settings();
    Ok(view! {
        edit_page(
            kind: "organization",
            subject: &stored.name,
            name: &org.name,
            description: &org.description,
            crumbs: &crumbs,
            action: org_edit_url(stored.id()),
            cancel: base,
            error: error,
            project_checklist(options: projects, noun: "organization", error: error)
            settings_fields(s: &settings)
        )
    })
}
