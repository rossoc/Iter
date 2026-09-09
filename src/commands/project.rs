//! `iter project`, plus the three top-level commands that also create a
//! project: `iter init`, `iter new` and `iter clone`.
//!
//! All four creation paths funnel through [`create_project_interactively`],
//! which is what keeps the validation from being four slightly different
//! validations -- and is why they live in one file rather than being split
//! by which clap subcommand reaches them.

use crate::app::App;
use crate::args::{ProjectCommand, ReportOpts};
use crate::commands::Run;
use crate::config::config;
use crate::db::Db;
use crate::db::Table;
use crate::error::{IterError, Result};
use crate::md_edit::edited;
use crate::models::Project;
use crate::reporting::{Header, ProjectInfo, Report, settings_of};
use crate::utils::output::list_names;
use crate::utils::report::{resolve_range, task_reports, total_minutes};
use crate::utils::resolve::{resolve_organization, resolve_project_or_current};
use crate::utils::workspace::teardown_session_config;
use crate::{git, scaffold, tmux};

/// The one way a project is created: opens `template` in the editor,
/// validates what comes back, lets `populate` turn the `base_path` it names
/// into an actual directory (creating it, copying another project's files
/// into it, running `git clone`), and only then inserts the row -- so a
/// project never names a directory that was never made, and a directory is
/// never made for a `base_path` the user changed in the buffer.
///
/// `cloned_from` only names the origin in the message; quitting the editor
/// without saving says so and creates nothing. Shared by `iter init`,
/// `iter new`, `iter clone` and `iter project new`, which is what keeps the
/// validation below from being four slightly different validations.
fn create_project_interactively(
    db: &Db,
    template: &Project,
    cloned_from: Option<&str>,
    populate: impl FnOnce(&str) -> Result<()>,
) -> Result<()> {
    edited(template, "project", "created", |mut project| {
        project.organization_id = template.organization_id; // never carried through the YAML
        if project.name.trim().is_empty() {
            return Err(IterError::EmptyProjectName);
        }
        if project.base_path.trim().is_empty() {
            return Err(IterError::EmptyBasePath);
        }
        // Absolutised here, once, for every path a project can arrive by. A
        // relative `base_path` names a different directory from every place
        // `iter` is later run -- a different worktree to create, a different
        // repo to ask whether it's one -- and an empty one, rejected above,
        // reads as the current directory throughout.
        project.base_path = scaffold::absolute_path(project.base_path.trim())?;
        populate(&project.base_path)?;
        db.insert(&project)?;
        Ok(match cloned_from {
            Some(source) => format!(
                "created project '{}' (cloned from '{source}')",
                project.name
            ),
            None => format!("created project '{}'", project.name),
        })
    })
}

/// A blank project template, seeded with `organization`'s defaults when one
/// was named. With none, the project keeps `Project::template`'s own
/// defaults -- belonging to an organization is optional throughout.
fn new_project_template(db: &Db, organization: Option<&str>) -> Result<Project> {
    let mut template = Project::template(&config().project);
    if let Some(name) = organization {
        template.inherit_from(&resolve_organization(db, name)?);
    }
    Ok(template)
}

pub(crate) fn init_cmd(app: &App, organization: Option<&str>) -> Result<()> {
    let db = &app.db;
    let base_path = scaffold::absolute_path(".")?;
    let mut template = new_project_template(db, organization)?;
    // Detected from disk, so it wins over an inherited default: an
    // organization saying "github" can't make a non-repo into one.
    template.github = git::is_git_repo(&base_path);
    template.base_path = base_path;
    create_project_interactively(db, &template, None, |_| Ok(()))
}

pub(crate) fn new_cmd(app: &App, path: &str, organization: Option<&str>) -> Result<()> {
    let db = &app.db;
    let base_path = scaffold::absolute_path(path)?;

    let mut template = new_project_template(db, organization)?;
    template.name = scaffold::dir_name(&base_path);
    template.github = git::is_git_repo(&base_path);
    template.base_path = base_path;

    // The directory is made from what comes *back* from the editor, not
    // from the argument: edit `base_path` in the buffer and it's the edited
    // one that gets created. Nothing exists before the edit, so quitting
    // without saving leaves nothing behind to clean up either.
    create_project_interactively(db, &template, None, |dest| {
        Ok(std::fs::create_dir_all(dest)?)
    })
}

/// `iter clone <source>`: clones an existing local project as a template
/// when `source` names one, and otherwise treats `source` as a git remote
/// URL / local repo path and clones that.
pub(crate) fn clone_cmd(app: &App, source: &str, organization: Option<&str>) -> Result<()> {
    let db = &app.db;
    match db.find_by_name::<Project>(source)? {
        // Pre-fill from the source project -- including its name,
        // deliberately, so the user has to change it before saving --
        // leaving `base_path` blank for them to fill in. The source's
        // tasks and sessions are never touched: they live in the DB,
        // keyed to the source's own id.
        Some(source_project) => {
            let mut template = source_project.clone();
            template.id = None;
            template.base_path = String::new();
            // Cloning a project means keeping *its* settings, so an
            // `--organization` here only changes which organization the
            // copy belongs to -- it doesn't re-seed the defaults.
            if let Some(name) = organization {
                template.organization_id = resolve_organization(db, name)?.id;
            }
            let files_from = source_project.base_path.clone();
            create_project_interactively(db, &template, Some(&source_project.name), |dest| {
                std::fs::create_dir_all(dest)?;
                scaffold::copy_dir_excluding_git(
                    std::path::Path::new(&files_from),
                    std::path::Path::new(dest),
                )
            })
        }
        // `git clone` creates the destination directory itself.
        None => {
            let mut template = new_project_template(db, organization)?;
            template.github = true; // it is a repo, by construction
            create_project_interactively(db, &template, Some(source), |dest| {
                git::clone_repo(source, dest)
            })
        }
    }
}

impl Run for ProjectCommand {
    fn run(&self, app: &App) -> Result<()> {
        match self {
            Self::New { organization } => project_new(app, organization.as_deref()),
            Self::Edit { name, organization } => {
                project_edit(app, name.as_deref(), organization.as_deref())
            }
            Self::Delete { name } => project_delete(app, name.as_deref()),
            Self::Info { name, report } => project_info(app, name.as_deref(), report),
            Self::List => project_list(app),
        }
    }
}

fn project_new(app: &App, organization: Option<&str>) -> Result<()> {
    let db = &app.db;
    let template = new_project_template(db, organization)?;
    create_project_interactively(db, &template, None, |_| Ok(()))
}

fn project_edit(app: &App, name: Option<&str>, organization: Option<&str>) -> Result<()> {
    let db = &app.db;
    let existing = resolve_project_or_current(db, name)?;
    let id = existing.id();
    // `--organization` moves the project; without it, membership (or the
    // lack of it) is carried through untouched.
    let organization_id = match organization {
        Some(name) => resolve_organization(db, name)?.id,
        None => existing.organization_id,
    };
    edited(&existing, "project", "updated", |mut project| {
        project.organization_id = organization_id; // never carried through the YAML
        db.update(id, &project)?;
        Ok(format!("updated project '{}'", project.name))
    })
}

fn project_delete(app: &App, name: Option<&str>) -> Result<()> {
    let db = &app.db;
    let project = resolve_project_or_current(db, name)?;
    let project_id = project.id();

    let mut session_configs = Vec::new();
    for task in db.tasks_for_project(project_id)? {
        if let Some(session_config) = db.find_session_config_by_task(task.id())? {
            session_configs.push(session_config);
        }
    }

    // Delete the project (cascades to its tasks and their session-configs)
    // before tearing down tmux/worktrees: if we're running inside one of
    // those tmux sessions, `kill-session` in the teardown takes down this
    // pane too, so anything after that point may never run.
    db.delete::<Project>(project_id)?;

    let tmux_sessions: Vec<&str> = session_configs
        .iter()
        .filter_map(|session_config| teardown_session_config(db, &project, session_config))
        .collect();
    println!("deleted project '{}'", project.name);

    // Killing is left to the very end for the same reason the row deletion
    // moved to the front: run this from inside one of these sessions and
    // the first `kill-session` takes this process with it. Inside the loop
    // above, that would strand every worktree after the first.
    for name in tmux_sessions {
        tmux::kill_session(name);
    }
    Ok(())
}

fn project_info(app: &App, name: Option<&str>, opts: &ReportOpts) -> Result<()> {
    let db = &app.db;
    let project = resolve_project_or_current(db, name)?;
    let project_id = project.id();
    let now = app.now;
    let range = resolve_range(opts, now)?;

    // The union of every task's sessions in the period -- this is what
    // makes working two of the project's tasks in parallel not
    // double-count.
    let (tasks, sessions) = task_reports(db, project_id, range, now)?;
    let total_minutes = total_minutes(&sessions, now);

    let info = ProjectInfo {
        header: Header::new(
            project.name.clone(),
            &project.description,
            settings_of(&project)?,
            range,
            total_minutes,
        ),
        tasks,
    };
    print!("{}", info.render(opts.format)?);
    Ok(())
}

fn project_list(app: &App) -> Result<()> {
    list_names::<Project>(&app.db)
}
