//! `iter organization` -- grouping projects and reporting across them.

use crate::app::App;
use crate::args::{OrganizationCommand, ReportOpts};
use crate::commands::Run;
use crate::config::config;
use crate::db::Table;
use crate::error::{IterError, Result};
use crate::md_edit::edited;
use crate::models::Organization;
use crate::reporting::{Header, OrganizationInfo, Report, settings_of};
use crate::utils::output::list_names;
use crate::utils::report::{project_reports, resolve_range, total_minutes};
use crate::utils::resolve::resolve_organization_or_current;

impl Run for OrganizationCommand {
    fn run(&self, app: &App) -> Result<()> {
        match self {
            Self::New => organization_new(app),
            Self::Edit { name } => organization_edit(app, name.as_deref()),
            Self::Delete { name } => organization_delete(app, name.as_deref()),
            Self::Info { name, report } => organization_info(app, name.as_deref(), report),
            Self::List => organization_list(app),
        }
    }
}

fn organization_new(app: &App) -> Result<()> {
    let db = &app.db;
    let template = Organization::template(&config().project);
    edited(&template, "organization", "created", |organization| {
        if organization.name.trim().is_empty() {
            return Err(IterError::EmptyOrganizationName);
        }
        db.insert(&organization)?;
        Ok(format!("created organization '{}'", organization.name))
    })
}

fn organization_edit(app: &App, name: Option<&str>) -> Result<()> {
    let db = &app.db;
    let existing = resolve_organization_or_current(db, name)?;
    let id = existing.id();
    edited(&existing, "organization", "updated", |organization| {
        db.update(id, &organization)?;
        Ok(format!("updated organization '{}'", organization.name))
    })
}

/// Deletes the organization row only. Its projects survive -- the schema's
/// `ON DELETE SET NULL` just clears their `organization_id`, leaving them in
/// the state any project without an organization is already in.
fn organization_delete(app: &App, name: Option<&str>) -> Result<()> {
    let db = &app.db;
    let organization = resolve_organization_or_current(db, name)?;
    let id = organization.id();
    let kept = db.projects_for_organization(id)?.len();

    db.delete::<Organization>(id)?;

    let orphaned = match kept {
        0 => String::new(),
        1 => " -- 1 project kept, now without an organization".to_string(),
        n => format!(" -- {n} projects kept, now without an organization"),
    };
    println!("deleted organization '{}'{orphaned}", organization.name);
    Ok(())
}

/// The organization's report for a period: every project that was worked
/// on in it, each project's tasks, and every session behind those -- hours,
/// statuses and messages all the way down. A project (or task) with no
/// session in the period is left out; `organization list` / `task list` are
/// the place for the full roster.
fn organization_info(app: &App, name: Option<&str>, opts: &ReportOpts) -> Result<()> {
    let db = &app.db;
    let organization = resolve_organization_or_current(db, name)?;
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

fn organization_list(app: &App) -> Result<()> {
    list_names::<Organization>(&app.db)
}
