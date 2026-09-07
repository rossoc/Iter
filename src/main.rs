mod args;
mod db;
mod error;
mod git;
mod github;
mod models;
mod process;
mod reporting;
mod scaffold;
mod tmux;
mod yaml_edit;

use args::{
    Args, Command, InternalCommand, OrganizationCommand, ProjectCommand, SessionCommand,
    TaskCommand,
};
use chrono::{Local, NaiveDate};
use clap::{CommandFactory, Parser};
use clap_complete::engine::CompletionCandidate;
use clap_complete::env::CompleteEnv;
use db::{Db, Repository};
use error::{IterError, Result};
use models::{Organization, Project, Session, SessionConfig, Task, TaskStatus};
use reporting::{
    DetailReport, MERGE_GAP_MINUTES, OrganizationReport, ProjectSummary, TaskEntry, TaskSummary,
    WeekdayReport, concat_messages, format_detail_report, format_organization_report,
    merged_total_minutes, minutes_to_hhmm, on_date, round_to_half_hour, weekday_averages,
};
use std::process::ExitCode;

// An id read back from the database is always `Some`; this names that
// invariant at every `.expect()` call site below instead of `.unwrap()`ing
// silently, per a fetched-row's id never legitimately being absent.
const ID_INVARIANT: &str = "a row loaded from the database always has an id";

fn cli() -> clap::Command {
    Args::command().name("iter")
}

fn main() -> ExitCode {
    // Shell-driven dynamic completion: when invoked as `COMPLETE=<shell> iter
    // ...` (which the shell does behind the scenes on every Tab press, once
    // `source <(COMPLETE=zsh iter)` has registered it)
    CompleteEnv::with_factory(cli).complete();

    let args = Args::parse();

    let result = match &args.command {
        Command::Init { organization } => init_cmd(organization.as_deref()),
        Command::New { path, organization } => new_cmd(path, organization.as_deref()),
        Command::Clone {
            source,
            organization,
        } => clone_cmd(source, organization.as_deref()),
        Command::Organization { action } => dispatch_organization(action),
        Command::Project { action } => dispatch_project(action.as_ref()),
        Command::Task { action } => dispatch_task(action.as_ref()),
        Command::Session { action } => dispatch_session(action),
        Command::Comment { task, message } => comment_cmd(task.as_deref(), message),
        Command::T => t_cmd(),
        Command::Internal { action } => match action {
            InternalCommand::Hook {
                event,
                tmux_session,
            } => hook_cmd(event, tmux_session),
        },
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn open_db() -> Db {
    Db::open(db::DB_FILE).unwrap_or_else(|e| {
        eprintln!("failed to open database at {}: {e}", db::DB_FILE);
        std::process::exit(1);
    })
}

/// Dynamic completer for arguments naming a project (`project edit/delete/info`,
/// `task new --project`). Attached via `ArgValueCompleter` in `args.rs`.
pub(crate) fn project_completer(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    let Some(current) = current.to_str() else {
        return Vec::new();
    };
    let db = open_db();
    let Ok(projects) = Repository::<Project>::list(&db) else {
        return Vec::new();
    };
    projects
        .into_iter()
        .map(|p| p.name)
        .filter(|n| n.starts_with(current))
        .map(CompletionCandidate::new)
        .collect()
}

/// Dynamic completer for arguments naming an organization.
pub(crate) fn organization_completer(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    let Some(current) = current.to_str() else {
        return Vec::new();
    };
    let db = open_db();
    let Ok(organizations) = Repository::<Organization>::list(&db) else {
        return Vec::new();
    };
    organizations
        .into_iter()
        .map(|o| o.name)
        .filter(|n| n.starts_with(current))
        .map(CompletionCandidate::new)
        .collect()
}

/// Dynamic completer for arguments naming a task as `<project>/<task>`.
pub(crate) fn task_completer(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    let Some(current) = current.to_str() else {
        return Vec::new();
    };
    let db = open_db();
    let Ok(projects) = Repository::<Project>::list(&db) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for p in projects {
        let Ok(tasks) = db.tasks_for_project(p.id.expect(ID_INVARIANT)) else {
            continue;
        };
        for t in tasks {
            let full = format!("{}/{}", p.name, t.name);
            if full.starts_with(current) {
                out.push(CompletionCandidate::new(full));
            }
        }
    }
    out
}

/// Splits `<project>/<task>` on the first `/`.
fn parse_task_ref(task_ref: &str) -> Result<(&str, &str)> {
    task_ref
        .split_once('/')
        .ok_or_else(|| IterError::InvalidTaskRef(task_ref.to_string()))
}

fn resolve_task(db: &Db, task_ref: &str) -> Result<(Project, Task)> {
    let (project_name, task_name) = parse_task_ref(task_ref)?;
    let project = db
        .find_project_by_name(project_name)?
        .ok_or_else(|| IterError::ProjectNotFound(project_name.to_string()))?;
    let task = db
        .find_task(project.id.expect(ID_INVARIANT), task_name)?
        .ok_or_else(|| IterError::TaskNotFound {
            project: project_name.to_string(),
            task: task_name.to_string(),
        })?;
    Ok((project, task))
}

/// Resolves the project + task for the tmux session this process is
/// running inside -- the same lookup `iter t` prints.
fn current_session_task(db: &Db) -> Result<(Project, Task)> {
    let name = tmux::current_session_name().ok_or(IterError::NotInTmux)?;
    let session_config = db
        .find_session_config_by_tmux_name(&name)?
        .ok_or_else(|| IterError::UntrackedTmuxSession(name.clone()))?;
    let task =
        Repository::<Task>::get(db, session_config.task_id)?.ok_or(IterError::OrphanSessionTask)?;
    let project =
        Repository::<Project>::get(db, task.project_id)?.ok_or(IterError::OrphanTaskProject)?;
    Ok((project, task))
}

/// Resolves an explicit `<project>/<task>` ref, or -- when none is given --
/// the task of the tmux session this process is running inside.
fn resolve_task_or_current(db: &Db, task_ref: Option<&str>) -> Result<(Project, Task)> {
    match task_ref {
        Some(r) => resolve_task(db, r),
        None => current_session_task(db),
    }
}

/// Resolves an explicit project name, or -- when none is given -- the
/// project of the tmux session this process is running inside.
fn resolve_project_or_current(db: &Db, name: Option<&str>) -> Result<Project> {
    match name {
        Some(n) => db
            .find_project_by_name(n)?
            .ok_or_else(|| IterError::ProjectNotFound(n.to_string())),
        None => current_session_task(db).map(|(project, _)| project),
    }
}

fn resolve_organization(db: &Db, name: &str) -> Result<Organization> {
    db.find_organization_by_name(name)?
        .ok_or_else(|| IterError::OrganizationNotFound(name.to_string()))
}

/// Resolves an explicit organization name, or -- when none is given -- the
/// organization of the tmux session's project. Erroring when that project
/// belongs to none is deliberate: there's no sensible "current"
/// organization to fall back to, and membership is optional by design.
fn resolve_organization_or_current(db: &Db, name: Option<&str>) -> Result<Organization> {
    if let Some(name) = name {
        return resolve_organization(db, name);
    }
    let project = current_session_task(db)?.0;
    let id = project
        .organization_id
        .ok_or_else(|| IterError::ProjectHasNoOrganization(project.name.clone()))?;
    Repository::<Organization>::get(db, id)?.ok_or(IterError::OrphanProjectOrganization)
}

fn parse_date_filter(date_filter: Option<&str>, now: chrono::NaiveDateTime) -> Result<NaiveDate> {
    match date_filter {
        Some(d) => NaiveDate::parse_from_str(d, "%Y-%m-%d")
            .map_err(|_| IterError::InvalidDate(d.to_string())),
        None => Ok(now.date()),
    }
}

fn print_detail_report(
    name: &str,
    description: &str,
    date: NaiveDate,
    sessions: &[Session],
    now: chrono::NaiveDateTime,
    tasks: Option<Vec<TaskEntry>>,
) {
    let total_minutes = merged_total_minutes(sessions, now, MERGE_GAP_MINUTES);
    let report = DetailReport {
        name: name.to_string(),
        description: Some(description.trim())
            .filter(|d| !d.is_empty())
            .map(str::to_string),
        date: date.format("%Y-%m-%d").to_string(),
        total_hours: round_to_half_hour(total_minutes as f64 / 60.0),
        total_hhmm: minutes_to_hhmm(total_minutes),
        messages: concat_messages(sessions),
        tasks,
    };
    print!("{}", format_detail_report(&report));
}

// ---- sessions: shared by manual start/stop and tmux hooks -----------------

fn start_session(db: &Db, task_id: i64) -> Result<()> {
    if db.open_session_for_task(task_id)?.is_some() {
        return Ok(()); // already has an open session; don't double-start
    }
    let session = Session {
        id: None,
        task_id,
        start: Local::now().naive_local(),
        end: None,
        message: None,
    };
    Repository::<Session>::insert(db, &session)?;
    Ok(())
}

fn close_open_session(db: &Db, task_id: i64, message: Option<&str>) -> Result<()> {
    if let Some(mut open) = db.open_session_for_task(task_id)? {
        debug_assert!(
            open.is_ongoing(),
            "open_session_for_task returned a closed session"
        );
        open.end = Some(Local::now().naive_local());
        if let Some(m) = message {
            open.message = Some(m.to_string());
        }
        Repository::<Session>::update(db, open.id.expect(ID_INVARIANT), &open)?;
    }
    Ok(())
}

/// Tears down a task's session-config: removes its worktree (if any),
/// deletes its branch (if any), deletes the session-config row, and finally
/// kills its tmux session (if any). Called from `task done`, `task delete`
/// and `project delete`.
///
/// The tmux kill comes last, and deliberately isn't the thing anything else
/// here depends on: if this is run from inside that same tmux session,
/// `kill-session` takes down every pane in it -- including this one -- so
/// nothing after that call is guaranteed to run.
fn teardown_session_config(db: &Db, project: &Project, session_config: &SessionConfig) {
    if let Some(worktree) = &session_config.worktree_path
        && let Err(e) = git::remove_worktree(&project.base_path, worktree)
    {
        eprintln!("warning: {e}");
    }
    if let Some(branch) = &session_config.github_branch
        && let Err(e) = git::delete_branch(&project.base_path, branch)
    {
        eprintln!("warning: {e}");
    }
    if let Some(id) = session_config.id
        && let Err(e) = Repository::<SessionConfig>::delete(db, id)
    {
        eprintln!("warning: failed to delete session-config row: {e}");
    }
    if let Some(name) = &session_config.tmux_session_name {
        tmux::kill_session(name);
    }
}

// ---- init / new / clone ---------------------------------------------------

/// Opens `template` in nvim; if saved (and named), inserts it as a new
/// project and returns `true`. Returns `false` (having printed a
/// "no changes" message) if the user quit without saving -- the shared
/// tail of `project new`, `iter init`, and `iter new`.
fn create_project_interactively(db: &Db, template: Project) -> Result<bool> {
    let organization_id = template.organization_id;
    match yaml_edit::edit_in_nvim(&template)? {
        Some(mut project) => {
            if project.name.trim().is_empty() {
                return Err(IterError::EmptyProjectName);
            }
            project.organization_id = organization_id; // never carried through the YAML
            Repository::<Project>::insert(db, &project)?;
            println!("created project '{}'", project.name);
            Ok(true)
        }
        None => {
            println!("no changes -- project not created");
            Ok(false)
        }
    }
}

/// A blank project template, seeded with `organization`'s defaults when one
/// was named. With none, the project keeps `Project::template`'s own
/// defaults -- belonging to an organization is optional throughout.
fn new_project_template(db: &Db, organization: Option<&str>) -> Result<Project> {
    let mut template = Project::template();
    if let Some(name) = organization {
        template.inherit_from(&resolve_organization(db, name)?);
    }
    Ok(template)
}

fn init_cmd(organization: Option<&str>) -> Result<()> {
    let db = open_db();
    let base_path = scaffold::absolute_path(".")?;
    let mut template = new_project_template(&db, organization)?;
    // Detected from disk, so it wins over an inherited default: an
    // organization saying "github" can't make a non-repo into one.
    template.github = git::is_git_repo(&base_path);
    template.base_path = base_path;
    create_project_interactively(&db, template)?;
    Ok(())
}

fn new_cmd(path: &str, organization: Option<&str>) -> Result<()> {
    let db = open_db();
    let base_path = scaffold::absolute_path(path)?;
    std::fs::create_dir_all(&base_path)?;

    let mut template = new_project_template(&db, organization)?;
    template.name = scaffold::dir_name(&base_path);
    template.github = git::is_git_repo(&base_path);
    template.base_path = base_path.clone();

    // If the user quits without saving, don't leave an empty folder behind.
    if !create_project_interactively(&db, template)? {
        scaffold::remove_dir_if_empty(&base_path);
    }
    Ok(())
}

/// `iter clone <source>`: clones an existing local project as a template
/// when `source` names one, and otherwise treats `source` as a git remote
/// URL / local repo path and clones that.
fn clone_cmd(source: &str, organization: Option<&str>) -> Result<()> {
    let db = open_db();
    match db.find_project_by_name(source)? {
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
                template.organization_id = resolve_organization(&db, name)?.id;
            }
            let files_from = source_project.base_path.clone();
            clone_into(&db, &template, &source_project.name, |dest| {
                std::fs::create_dir_all(dest)?;
                scaffold::copy_dir_excluding_git(
                    std::path::Path::new(&files_from),
                    std::path::Path::new(dest),
                )
            })
        }
        // `git clone` creates the destination directory itself.
        None => {
            let mut template = new_project_template(&db, organization)?;
            template.github = true; // it is a repo, by construction
            clone_into(&db, &template, source, |dest| git::clone_repo(source, dest))
        }
    }
}

/// The shared body of both clone flows: open `template` in the editor,
/// validate the saved result and resolve its `base_path` to an absolute
/// path, let `populate` turn that path into an actual, populated directory
/// (copying a template project's files vs. running `git clone`), then
/// insert the row. `source_label` only names the origin in the message.
fn clone_into(
    db: &Db,
    template: &Project,
    source_label: &str,
    populate: impl FnOnce(&str) -> Result<()>,
) -> Result<()> {
    let Some(mut project) = yaml_edit::edit_in_nvim(template)? else {
        println!("no changes -- project not created");
        return Ok(());
    };
    project.organization_id = template.organization_id; // never carried through the YAML
    if project.name.trim().is_empty() {
        return Err(IterError::EmptyProjectName);
    }
    if project.base_path.trim().is_empty() {
        return Err(IterError::EmptyBasePath);
    }
    let base_path = scaffold::absolute_path(&project.base_path)?;
    populate(&base_path)?;

    project.base_path = base_path;
    Repository::<Project>::insert(db, &project)?;
    println!(
        "created project '{}' (cloned from '{source_label}')",
        project.name
    );
    Ok(())
}

// ---- organization ----------------------------------------------------

fn dispatch_organization(action: &OrganizationCommand) -> Result<()> {
    match action {
        OrganizationCommand::New => organization_new(),
        OrganizationCommand::Edit { name } => organization_edit(name.as_deref()),
        OrganizationCommand::Delete { name } => organization_delete(name.as_deref()),
        OrganizationCommand::Info { name } => organization_info(name.as_deref()),
        OrganizationCommand::List => organization_list(),
    }
}

fn organization_new() -> Result<()> {
    let db = open_db();
    match yaml_edit::edit_in_nvim(&Organization::template())? {
        Some(organization) => {
            if organization.name.trim().is_empty() {
                return Err(IterError::EmptyOrganizationName);
            }
            Repository::<Organization>::insert(&db, &organization)?;
            println!("created organization '{}'", organization.name);
        }
        None => println!("no changes -- organization not created"),
    }
    Ok(())
}

fn organization_edit(name: Option<&str>) -> Result<()> {
    let db = open_db();
    let existing = resolve_organization_or_current(&db, name)?;
    let id = existing.id.expect(ID_INVARIANT);
    match yaml_edit::edit_in_nvim(&existing)? {
        Some(organization) => {
            Repository::<Organization>::update(&db, id, &organization)?;
            println!("updated organization '{}'", organization.name);
        }
        None => println!("no changes -- organization not updated"),
    }
    Ok(())
}

/// Deletes the organization row only. Its projects survive -- the schema's
/// `ON DELETE SET NULL` just clears their `organization_id`, leaving them in
/// the state any project without an organization is already in.
fn organization_delete(name: Option<&str>) -> Result<()> {
    let db = open_db();
    let organization = resolve_organization_or_current(&db, name)?;
    let id = organization.id.expect(ID_INVARIANT);
    let kept = db.projects_for_organization(id)?.len();

    Repository::<Organization>::delete(&db, id)?;

    match kept {
        0 => println!("deleted organization '{}'", organization.name),
        1 => println!(
            "deleted organization '{}' -- 1 project kept, now without an organization",
            organization.name
        ),
        n => println!(
            "deleted organization '{}' -- {n} projects kept, now without an organization",
            organization.name
        ),
    }
    Ok(())
}

/// The organization's roster: every project in it, and every one of those
/// projects' tasks with its status. No dates, times or messages -- see
/// `project info` / `task info` for those.
fn organization_info(name: Option<&str>) -> Result<()> {
    let db = open_db();
    let organization = resolve_organization_or_current(&db, name)?;
    let organization_id = organization.id.expect(ID_INVARIANT);

    let mut projects = Vec::new();
    for project in db.projects_for_organization(organization_id)? {
        let tasks = db
            .tasks_for_project(project.id.expect(ID_INVARIANT))?
            .into_iter()
            .map(|task| TaskSummary {
                name: task.name,
                status: task.status.as_str().to_string(),
            })
            .collect();
        projects.push(ProjectSummary {
            name: project.name,
            tasks,
        });
    }

    let report = OrganizationReport {
        name: organization.name,
        description: Some(organization.description.trim())
            .filter(|d| !d.is_empty())
            .map(str::to_string),
        projects,
    };
    print!("{}", format_organization_report(&report));
    Ok(())
}

fn organization_list() -> Result<()> {
    let db = open_db();
    let names: Vec<String> = Repository::<Organization>::list(&db)?
        .into_iter()
        .map(|o| o.name)
        .collect();
    print!("{}", serde_yaml::to_string(&names)?);
    Ok(())
}

// ---- project ---------------------------------------------------------

/// `iter project` with no subcommand is `iter project list` -- the listing
/// is what you want often enough that it's the bare command's meaning.
fn dispatch_project(action: Option<&ProjectCommand>) -> Result<()> {
    match action {
        None => project_list(),
        Some(ProjectCommand::New { organization }) => project_new(organization.as_deref()),
        Some(ProjectCommand::Edit { name, organization }) => {
            project_edit(name.as_deref(), organization.as_deref())
        }
        Some(ProjectCommand::Delete { name }) => project_delete(name.as_deref()),
        Some(ProjectCommand::Info { name, date }) => project_info(name.as_deref(), date.as_deref()),
        Some(ProjectCommand::List) => project_list(),
    }
}

fn project_new(organization: Option<&str>) -> Result<()> {
    let db = open_db();
    let template = new_project_template(&db, organization)?;
    create_project_interactively(&db, template)?;
    Ok(())
}

fn project_edit(name: Option<&str>, organization: Option<&str>) -> Result<()> {
    let db = open_db();
    let existing = resolve_project_or_current(&db, name)?;
    let id = existing.id.expect(ID_INVARIANT);
    // `--organization` moves the project; without it, membership (or the
    // lack of it) is carried through untouched.
    let organization_id = match organization {
        Some(name) => resolve_organization(&db, name)?.id,
        None => existing.organization_id,
    };
    match yaml_edit::edit_in_nvim(&existing)? {
        Some(mut project) => {
            project.organization_id = organization_id;
            Repository::<Project>::update(&db, id, &project)?;
            println!("updated project '{}'", project.name);
        }
        None => println!("no changes -- project not updated"),
    }
    Ok(())
}

fn project_delete(name: Option<&str>) -> Result<()> {
    let db = open_db();
    let project = resolve_project_or_current(&db, name)?;
    let project_id = project.id.expect(ID_INVARIANT);

    let mut session_configs = Vec::new();
    for task in db.tasks_for_project(project_id)? {
        if let Some(session_config) =
            db.find_session_config_by_task(task.id.expect(ID_INVARIANT))?
        {
            session_configs.push(session_config);
        }
    }

    // Delete the project (cascades to its tasks and their session-configs)
    // before tearing down tmux/worktrees: if we're running inside one of
    // those tmux sessions, `kill-session` in the teardown takes down this
    // pane too, so anything after that point may never run.
    Repository::<Project>::delete(&db, project_id)?;

    for session_config in &session_configs {
        teardown_session_config(&db, &project, session_config);
    }
    println!("deleted project '{}'", project.name);
    Ok(())
}

/// One `TaskEntry` per task of `project_id` that had a session on `date` --
/// the `tasks:` breakdown in a project's `info` report. Tasks untouched
/// that day are left out; a task's `messages` covers only that day's
/// sessions, same as the report's own date filter.
fn project_task_entries(db: &Db, project_id: i64, date: NaiveDate) -> Result<Vec<TaskEntry>> {
    let mut entries = Vec::new();
    for task in db.tasks_for_project(project_id)? {
        let sessions = on_date(db.sessions_for_task(task.id.expect(ID_INVARIANT))?, date);
        if sessions.is_empty() {
            continue;
        }
        entries.push(TaskEntry {
            name: task.name,
            status: task.status.as_str().to_string(),
            messages: concat_messages(&sessions),
        });
    }
    Ok(entries)
}

fn project_info(name: Option<&str>, date_filter: Option<&str>) -> Result<()> {
    let db = open_db();
    let project = resolve_project_or_current(&db, name)?;
    let project_id = project.id.expect(ID_INVARIANT);
    let now = Local::now().naive_local();
    let date = parse_date_filter(date_filter, now)?;

    // The union of every task's sessions that day -- this is what makes
    // working two of the project's tasks in parallel not double-count.
    let sessions = on_date(db.sessions_for_project(project_id)?, date);

    let tasks = project_task_entries(&db, project_id, date)?;

    print_detail_report(
        &project.name,
        &project.description,
        date,
        &sessions,
        now,
        Some(tasks),
    );
    Ok(())
}

fn project_list() -> Result<()> {
    let db = open_db();
    let names: Vec<String> = Repository::<Project>::list(&db)?
        .into_iter()
        .map(|p| p.name)
        .collect();
    print!("{}", serde_yaml::to_string(&names)?);
    Ok(())
}

// ---- task --------------------------------------------------------------

/// `iter task` with no subcommand lists the unfinished work: every task
/// still `queue` or `wip`, across every project. `task list` is the same
/// listing with the filters spelled out.
fn dispatch_task(action: Option<&TaskCommand>) -> Result<()> {
    match action {
        None => task_list(None, &[TaskStatus::Queue, TaskStatus::Wip]),
        Some(TaskCommand::New { project, issue }) => task_new(project.as_deref(), *issue),
        Some(TaskCommand::Edit { task }) => task_edit(task.as_deref()),
        Some(TaskCommand::Delete { task }) => task_delete(task.as_deref()),
        Some(TaskCommand::Info { task, date }) => task_info(task.as_deref(), date.as_deref()),
        Some(TaskCommand::List { project, status }) => {
            let statuses = match status {
                Some(s) => vec![
                    TaskStatus::parse(s).ok_or_else(|| IterError::InvalidStatus(s.to_string()))?,
                ],
                None => Vec::new(),
            };
            task_list(project.as_deref(), &statuses)
        }
        Some(TaskCommand::Done { task }) => task_done(task.as_deref()),
        Some(TaskCommand::Weekday { task }) => task_weekday(task.as_deref()),
    }
}

fn task_new(project_name: Option<&str>, issue: Option<i64>) -> Result<()> {
    let db = open_db();
    let project = resolve_project_or_current(&db, project_name)?;
    let project_id = project.id.expect(ID_INVARIANT);

    let mut template = Task::template(project_id, git::branch_prefix(&project.branch_template));
    if let Some(issue_number) = issue {
        if !project.github {
            return Err(IterError::GithubDisabled(project.name.clone()));
        }
        let info = github::fetch_issue(&project.base_path, issue_number)?;
        template.name = info.title;
        template.description = info.body;
        template.github_issue = Some(issue_number);
    }

    match yaml_edit::edit_in_nvim(&template)? {
        Some(mut task) => {
            if task.name.trim().is_empty() {
                return Err(IterError::EmptyTaskName);
            }
            task.project_id = project_id; // never carried through the YAML
            Repository::<Task>::insert(&db, &task)?;
            println!("created task '{}/{}'", project.name, task.name);
        }
        None => println!("no changes -- task not created"),
    }
    Ok(())
}

fn task_edit(task_ref: Option<&str>) -> Result<()> {
    let db = open_db();
    let (project, existing) = resolve_task_or_current(&db, task_ref)?;
    let id = existing.id.expect(ID_INVARIANT);
    let project_id = existing.project_id;
    let display = format!("{}/{}", project.name, existing.name);
    match yaml_edit::edit_in_nvim(&existing)? {
        Some(mut task) => {
            task.project_id = project_id;
            Repository::<Task>::update(&db, id, &task)?;
            println!("updated task '{display}'");
        }
        None => println!("no changes -- task not updated"),
    }
    Ok(())
}

fn task_delete(task_ref: Option<&str>) -> Result<()> {
    let db = open_db();
    let (project, task) = resolve_task_or_current(&db, task_ref)?;
    let task_id = task.id.expect(ID_INVARIANT);
    let display = format!("{}/{}", project.name, task.name);
    let session_config = db.find_session_config_by_task(task_id)?;

    // Delete the task row (cascades to its session-config) before tearing
    // down tmux/the worktree: if we're running inside the task's own tmux
    // session, `kill-session` in the teardown takes down this pane too, so
    // anything after that point may never run.
    Repository::<Task>::delete(&db, task_id)?;

    if let Some(session_config) = session_config {
        teardown_session_config(&db, &project, &session_config);
    }
    println!("deleted task '{display}'");
    Ok(())
}

fn task_info(task_ref: Option<&str>, date_filter: Option<&str>) -> Result<()> {
    let db = open_db();
    let (project, task) = resolve_task_or_current(&db, task_ref)?;
    let now = Local::now().naive_local();
    let date = parse_date_filter(date_filter, now)?;
    let sessions = on_date(db.sessions_for_task(task.id.expect(ID_INVARIANT))?, date);
    let display = format!("{}/{}", project.name, task.name);
    print_detail_report(&display, &task.description, date, &sessions, now, None);
    Ok(())
}

/// Prints `<project>/<task>` for every task matching both filters, as a
/// YAML list. An empty `statuses` means every status, the same way `None`
/// for `project_filter` means every project.
fn task_list(project_filter: Option<&str>, statuses: &[TaskStatus]) -> Result<()> {
    let db = open_db();

    let projects: Vec<Project> = Repository::<Project>::list(&db)?
        .into_iter()
        .filter(|p| project_filter.map(|f| f == p.name).unwrap_or(true))
        .collect();

    let mut names = Vec::new();
    for p in projects {
        let tasks = db.tasks_for_project(p.id.expect(ID_INVARIANT))?;
        for t in tasks {
            if statuses.is_empty() || statuses.contains(&t.status) {
                names.push(format!("{}/{}", p.name, t.name));
            }
        }
    }
    print!("{}", serde_yaml::to_string(&names)?);
    Ok(())
}

fn task_done(task_ref: Option<&str>) -> Result<()> {
    let db = open_db();
    let (project, mut task) = resolve_task_or_current(&db, task_ref)?;
    let task_id = task.id.expect(ID_INVARIANT);
    let display = format!("{}/{}", project.name, task.name);

    close_open_session(&db, task_id, None)?;

    // Persist the status change before tearing down the session-config: if
    // we're running inside the task's own tmux session, `kill-session` below
    // takes down every pane in it -- including this one -- so anything after
    // that point may never run.
    task.status = TaskStatus::Done;
    Repository::<Task>::update(&db, task_id, &task)?;

    if let Some(session_config) = db.find_session_config_by_task(task_id)? {
        teardown_session_config(&db, &project, &session_config);
    }

    println!("task '{display}' marked done");
    Ok(())
}

fn task_weekday(task_ref: Option<&str>) -> Result<()> {
    let db = open_db();
    let (project, task) = resolve_task_or_current(&db, task_ref)?;
    let now = Local::now().naive_local();
    let sessions = db.sessions_for_task(task.id.expect(ID_INVARIANT))?;
    let report = WeekdayReport {
        name: format!("{}/{}", project.name, task.name),
        weekdays: weekday_averages(&sessions, now),
    };
    print!("{}", serde_yaml::to_string(&report)?);
    Ok(())
}

// ---- session -------------------------------------------------------------

fn dispatch_session(action: &SessionCommand) -> Result<()> {
    match action {
        SessionCommand::New {
            task,
            branch,
            no_branch,
        } => session_new(task, branch.as_deref(), *no_branch),
        SessionCommand::Start { task } => session_start(task.as_deref()),
        SessionCommand::Stop { task, message } => session_stop(task.as_deref(), message.as_deref()),
        SessionCommand::Elapse => session_elapse(),
    }
}

fn session_new(task_ref: &str, branch_override: Option<&str>, no_branch: bool) -> Result<()> {
    let db = open_db();
    let (project, mut task) = resolve_task(&db, task_ref)?;
    let task_id = task.id.expect(ID_INVARIANT);

    if db.find_session_config_by_task(task_id)?.is_some() {
        return Err(IterError::SessionAlreadyExists(task_ref.to_string()));
    }

    let tmux_session_name = format!(
        "{}/{}",
        project.name.replace('/', "-"),
        task.name.replace('/', "-")
    );

    if project.github && !git::is_git_repo(&project.base_path) {
        return Err(IterError::NotAGitRepo {
            name: project.name.clone(),
            path: project.base_path.clone(),
        });
    }

    if project.tmux && tmux::session_exists(&tmux_session_name) {
        return Err(IterError::TmuxSessionExists(tmux_session_name));
    }

    let mut github_branch = None;
    let mut worktree_path = None;

    if project.github && project.auto_branch && !no_branch {
        // The task's own prefix is what it was created (and possibly
        // edited) with; an empty one -- a task from before the field
        // existed, or one the user blanked -- falls back to whatever the
        // project's template says now.
        let prefix = match task.branch_prefix.trim() {
            "" => git::branch_prefix(&project.branch_template),
            prefix => prefix.to_string(),
        };
        let branch = branch_override
            .map(|b| b.to_string())
            .unwrap_or_else(|| git::branch_name(&prefix, &task.name));
        let slug = git::slugify(&task.name);
        let path = std::path::Path::new(&project.base_path)
            .join(".iter-worktrees")
            .join(&slug);
        let path_str = path.to_string_lossy().to_string();
        git::create_worktree(&project.base_path, &branch, &path_str)?;
        github_branch = Some(branch);
        worktree_path = Some(path_str);
    }

    // Borrowed, not cloned: `cwd` only needs to outlive this function, and
    // both `worktree_path` and `project.base_path` already do.
    let cwd: &str = worktree_path.as_deref().unwrap_or(&project.base_path);

    let final_tmux_name = if project.tmux {
        tmux::create_session(&tmux_session_name, cwd)?;
        let iter_bin = std::env::current_exe()?.to_string_lossy().to_string();
        tmux::ensure_hooks_installed(&iter_bin)?;
        println!("session '{tmux_session_name}' started");
        Some(tmux_session_name)
    } else {
        println!(
            "session started for '{task_ref}' at {cwd} (tmux disabled for this project -- use `iter session start`/`iter session stop`)"
        );
        None
    };

    let session_config = SessionConfig {
        id: None,
        task_id,
        tmux_session_name: final_tmux_name,
        github_branch,
        worktree_path,
    };
    Repository::<SessionConfig>::insert(&db, &session_config)?;

    task.status = TaskStatus::Wip;
    Repository::<Task>::update(&db, task_id, &task)?;

    // Attaching last, and only once every row is written: it hands the
    // terminal over to tmux until the user detaches, and the
    // `client-attached` hook that fires on the way in looks the
    // session-config up by tmux name -- so the row has to be there first.
    if let Some(name) = &session_config.tmux_session_name
        && let Err(e) = tmux::attach_session(name)
    {
        eprintln!("warning: couldn't attach to '{name}': {e}");
        eprintln!("the session is set up -- attach with: tmux attach -t '{name}'");
    }

    Ok(())
}

fn session_start(task_ref: Option<&str>) -> Result<()> {
    let db = open_db();
    let (project, task) = resolve_task_or_current(&db, task_ref)?;
    start_session(&db, task.id.expect(ID_INVARIANT))?;
    println!("started a session for '{}/{}'", project.name, task.name);
    Ok(())
}

fn session_stop(task_ref: Option<&str>, message: Option<&str>) -> Result<()> {
    let db = open_db();
    let (project, task) = resolve_task_or_current(&db, task_ref)?;
    close_open_session(&db, task.id.expect(ID_INVARIANT), message)?;
    println!("stopped the session for '{}/{}'", project.name, task.name);
    Ok(())
}

/// Time elapsed on the open session of the task of the tmux session this
/// process is running in -- the session `start_session` created when the
/// client attached (see `hook_cmd`).
fn session_elapse() -> Result<()> {
    let db = open_db();
    let (project, task) = current_session_task(&db)?;
    let task_id = task.id.expect(ID_INVARIANT);
    let display = format!("{}/{}", project.name, task.name);
    let session = db
        .open_session_for_task(task_id)?
        .ok_or_else(|| IterError::NoOpenSession(display))?;
    let now = Local::now().naive_local();
    println!("{}", minutes_to_hhmm(session.duration_minutes(now)));
    Ok(())
}

// ---- comment / t / internal hook -----------------------------------------

fn comment_cmd(task_ref: Option<&str>, message: &str) -> Result<()> {
    let db = open_db();
    let (project, task) = resolve_task_or_current(&db, task_ref)?;
    if !project.github {
        return Err(IterError::GithubDisabled(project.name));
    }
    let issue = task
        .github_issue
        .ok_or_else(|| IterError::NoLinkedIssue(format!("{}/{}", project.name, task.name)))?;
    github::post_comment(&project.base_path, issue, message)?;
    println!("posted comment on issue #{issue}");
    Ok(())
}

fn t_cmd() -> Result<()> {
    let db = open_db();
    let (project, task) = current_session_task(&db)?;
    println!("{}/{}", project.name, task.name);
    Ok(())
}

fn hook_cmd(event: &str, tmux_session: &str) -> Result<()> {
    let db = open_db();
    let Some(session_config) = db.find_session_config_by_tmux_name(tmux_session)? else {
        return Ok(()); // not one of ours -- ignore
    };
    match event {
        "client-attached" => start_session(&db, session_config.task_id),
        "client-detached" | "session-closed" => {
            close_open_session(&db, session_config.task_id, None)
        }
        _ => Ok(()),
    }
}
