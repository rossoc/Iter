//! The organization edit form (`/org/{id}/edit`): the routes and the view.
//! The page is `edit_page`; this supplies the crumbs and the controls of its
//! own (the Projects checklist is the board form's, built by
//! `project_options`).

use super::daily_notes::NotesTab;
use super::edit::{Loaded, load, submit};
use super::edit_page::{edit_page, settings_row};
use super::forms::OrgForm;
use super::org_report::tab_url;
use super::platform_pick::{self, platform_lists};
use super::project_options::{group_options, project_checklist};
use super::settings_fields::settings_fields;
use super::ui::breadcrumb::Crumb;
use super::ui::field::{CheckOption, field, path_field};
use super::ui::form_error::FormError;
use super::url::{notes_tab_url, org_edit_url, org_url, projects_tab_url, tasks_tab_url};
use super::{Id, open_db};
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
    let crumbs = [Crumb::label("Organization")];
    let settings = org.settings();
    // the badges the organization page itself shows
    let db = open_db()?;
    let (projects_n, tasks_n) = (
        db.projects_in::<Organization>(stored.id())?.len(),
        db.count_tasks_in::<Organization>(stored.id())?,
    );
    let key_hint = format!(
        "Where hours are pushed to. API key: {}. The key is never shown here; set or clear it with `iter org key {}`.",
        if crate::secret::is_set(&crate::secret::Keychain, &stored.name) {
            "set"
        } else {
            "not set"
        },
        stored.name
    );
    // what the platform offers, when there is a URL and a key to ask with
    let (platform_projects, platform_activities) = platform_pick::lists(org, &org.platform_project);
    let activities_url = format!("{base}/platform-activities");
    let notes = NotesTab::load(crate::notes::org_dir(stored), false);
    let sections = settings_row(
        &base,
        &[
            ("Info", base.clone(), None),
            ("Projects", projects_tab_url(&base), Some(projects_n)),
            ("Tasks", tasks_tab_url(&base), Some(tasks_n)),
            ("Notes", notes_tab_url(&base), NotesTab::badge(Some(&notes))),
            ("Report", tab_url(&base, super::now().date()), None),
        ],
    );
    Ok(view! {
        edit_page(
            kind: "organization",
            subject: &stored.name,
            name: &org.name,
            description: &org.description,
            crumbs: &crumbs,
            sections: Some(&sections),
            action: org_edit_url(stored.id()),
            cancel: base,
            error: error,
            path_field(
                name: OrgForm::NOTES_PATH,
                label: "Notes path",
                value: &org.notes_path,
                hint: "Where the organization's daily notes are kept, and the folder its projects without a notes path of their own keep theirs in.",
                error: error
            )
            field(
                name: OrgForm::PLATFORM_URL,
                label: "Time platform URL",
                value: &org.platform_url,
                hint: &key_hint,
                identifier: true,
                error: error
            )
            field(
                name: OrgForm::PLATFORM_PROJECT,
                label: "Time platform project",
                value: &org.platform_project,
                hint: "The platform project hours are logged under. Defaults to this organization's name; can be changed on each share. Suggestions come from the platform when it answers.",
                list: platform_pick::LIST_PROJECTS,
                identifier: true,
                error: error
            )
            field(
                name: OrgForm::PLATFORM_ACTIVITY,
                label: "Time platform activity",
                value: &org.platform_activity,
                hint: "The activity hours are logged under. Can be changed on each share. Suggestions are the activities of the project above.",
                list: platform_pick::LIST_ACTIVITIES,
                identifier: true,
                error: error
            )
            platform_lists(url: &activities_url, projects: &platform_projects, activities: &platform_activities)
            project_checklist(options: projects, noun: "organization", error: error)
            settings_fields(s: &settings)
        )
    })
}
