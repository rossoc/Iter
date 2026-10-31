//! `iter organization` -- grouping projects and reporting across them.

use crate::app::App;
use crate::args::{OrganizationCommand, ReportOpts};
use crate::commands::Run;
use crate::config::config;
use crate::db::Table;
use crate::error::{IterError, Result};
use crate::models::Organization;
use crate::platform::JsonWebhook;
use crate::process::run_status;
use crate::github::GhCli;
use crate::push::{self, Target};
use crate::reporting::{Header, OrganizationInfo, print_report};
use crate::secret::{self, KeyStore, Keychain, Secret};
use crate::utils::crud::{create, delete_group, update_group};
use crate::utils::output::list_names;
use crate::utils::report::{project_reports, resolve_range, total_minutes};
use crate::utils::resolve::resolve_group_or_current;
use std::io::IsTerminal;

impl Run for OrganizationCommand {
    fn run(&self, app: &App) -> Result<()> {
        match self {
            Self::New => create(&app.db, &Organization::template(&config().project)),
            Self::Edit { name } => {
                let existing = organization(app, name)?;
                let (id, old) = (existing.id(), existing.name.clone());
                update_group(&app.db, existing)?;
                // an edit that was cancelled or failed has the same name
                if let Some(now) = app.db.get::<Organization>(id)? {
                    secret::rename_key(&Keychain, &old, &now.name)?;
                }
                Ok(())
            }
            Self::Delete { name } => {
                let existing = organization(app, name)?;
                delete_group(&app.db, &existing)?;
                // the key is kept by name: with the organization gone, nothing
                // could reach it again (best effort: the row is already gone)
                let _ = Keychain.delete(&existing.name);
                Ok(())
            }
            Self::Info { name, report } => organization_info(app, name.as_deref(), report),
            Self::Key { name, clear } => org_key(app, name.as_deref(), *clear),
            Self::Push {
                name,
                report,
                dry_run,
                force,
                platform_organization,
                activity,
            } => org_push(
                app,
                name.as_deref(),
                report,
                PushFlags {
                    dry_run: *dry_run,
                    force: *force,
                    organization: platform_organization.as_deref(),
                    activity: activity.as_deref(),
                },
            ),
            Self::List => list_names::<Organization>(&app.db),
        }
    }
}

/// An explicit organization name, or -- when none is given -- the one the
/// tmux session's project is in.
fn organization(app: &App, name: &Option<String>) -> Result<Organization> {
    resolve_group_or_current(&app.db, name.as_deref())
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

    let (projects, all_sessions) = project_reports(
        db,
        &db.projects_in::<Organization>(organization_id)?,
        range,
        now,
    )?;

    // The union across every project, so an hour spent switching between
    // two of them isn't counted twice at the organization level either.
    let total_minutes = total_minutes(&all_sessions, now);
    let info = OrganizationInfo {
        header: Header::of(
            organization.name.clone(),
            &organization.description,
            &organization,
            range,
            total_minutes,
        )?,
        projects,
    };
    print_report(&info, opts.format)
}

/// Sets the organization's API key from the first line of stdin (prompting
/// without echo on a terminal), or removes it. The key is never an argument.
fn org_key(app: &App, name: Option<&str>, clear: bool) -> Result<()> {
    let organization = resolve_group_or_current::<Organization>(&app.db, name)?;
    if clear {
        Keychain.delete(&organization.name)?;
        println!("Cleared the API key of '{}'.", organization.name);
        return Ok(());
    }
    let key = read_secret(&format!("API key for '{}': ", organization.name))?;
    let key = Secret::new(&key).ok_or_else(|| IterError::CommandFailed("no key given".into()))?;
    Keychain.set(&organization.name, &key)?;
    println!(
        "Stored the API key of '{}' in the keychain.",
        organization.name
    );
    Ok(())
}

/// One line from stdin; on a terminal, with a prompt and echo off.
fn read_secret(prompt: &str) -> Result<String> {
    let tty = std::io::stdin().is_terminal();
    if tty {
        eprint!("{prompt}");
        let _ = run_status("stty", None, &["-echo"]);
    }
    let mut line = String::new();
    let read = std::io::stdin().read_line(&mut line);
    if tty {
        let _ = run_status("stty", None, &["echo"]);
        eprintln!();
    }
    read?;
    Ok(line)
}

/// Asks a yes/no question on a terminal; without one the answer is no.
fn confirm(question: &str) -> bool {
    if !std::io::stdin().is_terminal() {
        return false;
    }
    eprint!("{question} [y/N] ");
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer).is_ok() && answer.trim().eq_ignore_ascii_case("y")
}

/// `text` in red, on a terminal.
fn red(text: &str) -> String {
    match std::io::stdout().is_terminal() {
        true => format!("\x1b[31m{text}\x1b[0m"),
        false => text.to_string(),
    }
}

struct PushFlags<'a> {
    dry_run: bool,
    force: bool,
    organization: Option<&'a str>,
    activity: Option<&'a str>,
}

fn org_push(app: &App, name: Option<&str>, opts: &ReportOpts, flags: PushFlags) -> Result<()> {
    let db = &app.db;
    let organization: Organization = resolve_group_or_current(db, name)?;
    if organization.platform_url.trim().is_empty() {
        return Err(IterError::CommandFailed(format!(
            "organization '{}' has no time platform URL: set platform_url with `iter org edit`",
            organization.name
        )));
    }
    let range = resolve_range(opts, app.now)?;
    let mut target = Target::of(&organization);
    if let Some(value) = flags.organization {
        target.organization = value.to_string();
    }
    if let Some(value) = flags.activity {
        target.activity = value.to_string();
    }
    let plan = push::plan(db, &organization, range, app.now, &target)?;

    // The platform is asked what it already holds (read-only), so a dry run
    // shows the conflicts too -- when there is a key to ask with.
    let platform = secret::resolve_env(&Keychain, &organization.name)
        .map(|key| JsonWebhook::new(&organization.platform_url, key));
    let check = match (&platform, flags.dry_run) {
        (Ok(platform), false) => Some(push::check(db, &organization, &plan, platform)?),
        // A preview is still worth showing when the platform cannot be asked.
        (Ok(platform), true) => match push::check(db, &organization, &plan, platform) {
            Ok(check) => Some(check),
            Err(e) => {
                eprintln!("warning: {e}");
                None
            }
        },
        (Err(_), true) => None,
        (Err(e), false) => return Err(IterError::Platform(e.to_string())),
    };

    print!("{}", plan.preview());
    if let Ok(platform) = &platform {
        for warning in push::unknown_targets(platform, &target) {
            eprintln!("warning: {warning}");
        }
    }
    if let Some(check) = &check {
        for line in check.conflict_lines() {
            println!("{}", red(&format!("CONFLICT {line}")));
        }
    }
    if flags.dry_run {
        if check.is_none() {
            println!("\nThe platform was not asked what it already holds (no API key, or it did not answer).");
        }
        println!("\nDry run: nothing was sent.");
        return Ok(());
    }

    // Past the dry run, a missing key or a failed check has already returned.
    let (Ok(platform), Some(check)) = (&platform, &check) else {
        return Err(IterError::Platform("the platform was not asked".into()));
    };
    if let Some(line) = check.already_pushed_line()
        && !flags.force
    {
        println!("\n{line}");
        if !confirm("Push anyway?") {
            return Err(IterError::CommandFailed(
                "not pushed (use --force to push without asking)".into(),
            ));
        }
    }
    let items = push::execute(db, &organization, &plan, check, platform, &GhCli::system(), app.now)?;
    println!();
    for item in &items {
        match item.error {
            Some(_) => println!("{}", red(&item.line())),
            None => println!("{}", item.line()),
        }
    }
    match items.iter().any(|i| i.error.is_some()) {
        true => Err(IterError::CommandFailed(
            "some steps failed or were skipped (see above)".into(),
        )),
        false => Ok(()),
    }
}
