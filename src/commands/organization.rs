//! `iter organization` -- grouping projects and reporting across them.

use crate::app::App;
use crate::args::{OrganizationCommand, ReportOpts};
use crate::commands::Run;
use crate::config::config;
use crate::db::Table;
use crate::error::Result;
use crate::md_edit::edited;
use crate::models::{Named, Organization, OrganizationEdit, Project};
use crate::reporting::{Header, OrganizationInfo, Report, settings_of};
use crate::utils::crud::{create, delete_group};
use crate::utils::output::list_names;
use crate::utils::report::{project_reports, resolve_range, total_minutes};
use crate::utils::resolve::resolve_group_or_current;

impl Run for OrganizationCommand {
    fn run(&self, app: &App) -> Result<()> {
        match self {
            Self::New => create(&app.db, &Organization::template(&config().project)),
            Self::Edit { name } => organization_edit(app, name.as_deref()),
            Self::Delete { name } => {
                let organization: Organization =
                    resolve_group_or_current(&app.db, name.as_deref())?;
                delete_group(&app.db, &organization)
            }
            Self::Info { name, report } => organization_info(app, name.as_deref(), report),
            Self::List => list_names::<Organization>(&app.db),
        }
    }
}

/// Edits the organization and its roster together: the buffer lists the
/// projects in it by name, and saving makes that list the membership.
fn organization_edit(app: &App, name: Option<&str>) -> Result<()> {
    let db = &app.db;
    let existing: Organization = resolve_group_or_current(db, name)?;
    let id = existing.id();
    let projects = db
        .projects_in::<Organization>(id)?
        .into_iter()
        .map(|p| p.name)
        .collect();
    let template = OrganizationEdit {
        organization: existing,
        projects,
    };
    edited(&template, Organization::KIND, "updated", |edit| {
        let mut organization = edit.organization;
        organization.validate()?;
        // Resolved before anything is written, so an unknown name leaves
        // the organization untouched.
        let project_ids = db.ids_by_name::<Project>(&edit.projects)?;
        db.update(id, &organization)?;
        db.set_projects::<Organization>(id, &project_ids)?;
        Ok(format!("updated organization '{}'", organization.name))
    })
}

/// The organization's report for a period: every project that was worked
/// on in it, each project's tasks, and every session behind those -- hours,
/// statuses and messages all the way down. A project (or task) with no
/// session in the period is left out; `organization list` / `task list` are
/// the place for the full roster.
fn organization_info(app: &App, name: Option<&str>, opts: &ReportOpts) -> Result<()> {
    let db = &app.db;
    let organization: Organization = resolve_group_or_current(db, name)?;
    let organization_id = organization.id();
    let now = app.now;
    let range = resolve_range(opts, now)?;

    let (projects, all_sessions) = project_reports(db, organization_id, range, now)?;

    // The union across every project, so an hour spent switching between
    // two of them isn't counted twice at the organization level either.
    let total_minutes = total_minutes(&all_sessions, now);
    let info = OrganizationInfo {
        header: Header::new(
            organization.name.clone(),
            &organization.description,
            settings_of(&organization)?,
            range,
            total_minutes,
        ),
        projects,
    };
    print!("{}", info.render(opts.format)?);
    Ok(())
}
