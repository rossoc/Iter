//! `iter project`, plus `iter init`, `iter new` and `iter clone`, which also create a project, all through [`create_project_interactively`] (one validation).

use crate::app::App;
use crate::args::{ProjectCommand, ReportOpts};
use crate::commands::Run;
use crate::config::config;
use crate::db::Db;
use crate::db::Table;
use crate::error::Result;
use crate::md_edit::edited;
use crate::models::{Board, Named, Organization, Project};
use crate::reporting::{Header, ProjectInfo, print_report};
use crate::utils::crud::update;
use crate::utils::output::list_names;
use crate::utils::report::{resolve_range, task_reports, total_minutes};
use crate::utils::resolve::resolve_project_or_current;
use crate::utils::workspace::teardown_session_config;
use crate::{git, scaffold, tmux};

/// The one way a project is created: opens `template` in the editor, validates, lets `populate` make the directory the edited `base_path` names, and only then inserts the row;
/// so no project names a directory that was never made, and none is made for a `base_path` the user changed. `cloned_from` only names the origin in the message.
fn create_project_interactively(
    db: &Db,
    template: &Project,
    cloned_from: Option<&str>,
    populate: impl FnOnce(&str) -> Result<()>,
) -> Result<()> {
    edited(template, Project::KIND, "created", |mut project| {
        // Also absolutises `base_path` -- see `Project::check`.
        project.validate()?;
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

/// A blank project template seeded with `organization`'s defaults when one was named.
fn new_project_template(db: &Db, organization: Option<&str>) -> Result<Project> {
    let mut template = Project::template(&config().project);
    if let Some(name) = organization {
        template.inherit_from(&db.resolve::<Organization>(name)?);
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

    // the directory is made from what comes back from the editor, not from the argument; quitting without saving leaves nothing to clean up
    create_project_interactively(db, &template, None, |dest| {
        Ok(std::fs::create_dir_all(dest)?)
    })
}

/// `iter clone <source>`: clones a local project as a template when `source` names one, else clones `source` as a git remote or local repo.
/// The clone stays on that project's board unless `--board` or `--no-board` says otherwise.
pub(crate) fn clone_cmd(
    app: &App,
    source: &str,
    organization: Option<&str>,
    board: Option<&str>,
    no_board: bool,
) -> Result<()> {
    let db = &app.db;
    match db.find_by_name::<Project>(source)? {
        // pre-fill from the source, name included so the user must change it; `base_path` is left blank. Its tasks and sessions are never touched
        Some(source_project) => {
            let mut template = source_project.clone();
            template.id = None;
            template.base_path = String::new();
            // cloning keeps the source's settings: `--organization` only moves the copy, it does not re-seed defaults
            if let Some(name) = organization {
                template.organization_id = db.resolve::<Organization>(name)?.id;
            }
            template.board_id = chosen_board(db, board, no_board, source_project.board_id)?;
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
            template.board_id = chosen_board(db, board, no_board, None)?;
            create_project_interactively(db, &template, Some(source), |dest| {
                git::clone_repo(source, dest)
            })
        }
    }
}

fn chosen_board(
    db: &Db,
    board: Option<&str>,
    no_board: bool,
    current: Option<i64>,
) -> Result<Option<i64>> {
    Ok(match (board, no_board) {
        (Some(name), _) => db.resolve::<Board>(name)?.id,
        (None, true) => None,
        (None, false) => current,
    })
}

impl Run for ProjectCommand {
    fn run(&self, app: &App) -> Result<()> {
        match self {
            Self::New {
                organization,
                board,
            } => project_new(app, organization.as_deref(), board.as_deref()),
            Self::Edit {
                name,
                organization,
                board,
                no_board,
            } => project_edit(
                app,
                name.as_deref(),
                organization.as_deref(),
                board.as_deref(),
                *no_board,
            ),
            Self::Delete { name } => project_delete(app, name.as_deref()),
            Self::Info { name, report } => project_info(app, name.as_deref(), report),
            Self::List => project_list(app),
        }
    }
}

fn project_new(app: &App, organization: Option<&str>, board: Option<&str>) -> Result<()> {
    let db = &app.db;
    let mut template = new_project_template(db, organization)?;
    if let Some(name) = board {
        template.board_id = db.resolve::<Board>(name)?.id;
    }
    create_project_interactively(db, &template, None, |_| Ok(()))
}

fn project_edit(
    app: &App,
    name: Option<&str>,
    organization: Option<&str>,
    board: Option<&str>,
    no_board: bool,
) -> Result<()> {
    let db = &app.db;
    let stored = resolve_project_or_current(db, name)?;
    let mut existing = stored.clone();
    // `--organization` moves the project; without it membership is carried through
    if let Some(name) = organization {
        existing.organization_id = db.resolve::<Organization>(name)?.id;
    }
    // likewise the board; `--no-board` is the one way to leave it
    existing.board_id = chosen_board(db, board, no_board, existing.board_id)?;
    // neither is in the buffer, so a move is saved even if the buffer is untouched
    let moved =
        (existing.organization_id, existing.board_id) != (stored.organization_id, stored.board_id);
    update(db, &existing, moved, |_| Ok(()))
}

fn project_delete(app: &App, name: Option<&str>) -> Result<()> {
    let db = &app.db;
    let project = resolve_project_or_current(db, name)?;
    let project_id = project.id();

    let session_configs = db.session_configs_for_project(project_id)?;

    // delete the row before tearing down tmux/worktrees: from inside one of those sessions `kill-session` takes this pane down too
    db.delete::<Project>(project_id)?;

    let tmux_sessions: Vec<&str> = session_configs
        .iter()
        .filter_map(|session_config| teardown_session_config(db, &project, session_config))
        .collect();
    println!("deleted project '{}'", project.name);

    // killing goes last for the same reason: the first `kill-session` would take this process with it and strand later worktrees
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

    // the union of every task's sessions, so two tasks worked in parallel are not double-counted
    let (tasks, sessions) = task_reports(db, project_id, range, now)?;
    let total_minutes = total_minutes(&sessions, now);

    let info = ProjectInfo {
        header: Header::of(
            project.name.clone(),
            &project.description,
            &project,
            range,
            total_minutes,
        )?,
        tasks,
    };
    print_report(&info, opts.format)
}

fn project_list(app: &App) -> Result<()> {
    list_names::<Project>(&app.db)
}
