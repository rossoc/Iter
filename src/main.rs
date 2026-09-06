mod args;
mod db;
mod error;
mod git;
mod github;
mod models;
mod reporting;
mod tmux;
mod yaml_edit;

use args::{Args, Command, InternalCommand, ProjectCommand, SessionCommand, TaskCommand};
use chrono::{Local, NaiveDate};
use clap::{CommandFactory, Parser};
use clap_complete::engine::CompletionCandidate;
use clap_complete::env::CompleteEnv;
use db::{Db, Repository};
use error::{IterError, Result};
use models::{Project, Record, Session, Task, TaskStatus};
use reporting::{
    DetailReport, MERGE_GAP_MINUTES, WeekdayReport, concat_messages, format_detail_report,
    merged_total_minutes, minutes_to_hhmm, round_to_half_hour, weekday_averages,
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
        Command::Project { action } => dispatch_project(action),
        Command::Task { action } => dispatch_task(action),
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
    let session = db
        .find_session_by_tmux_name(&name)?
        .ok_or_else(|| IterError::UntrackedTmuxSession(name.clone()))?;
    let task = Repository::<Task>::get(db, session.task_id)?.ok_or(IterError::OrphanSessionTask)?;
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

fn parse_date_filter(date_filter: Option<&str>, now: chrono::NaiveDateTime) -> Result<NaiveDate> {
    match date_filter {
        Some(d) => NaiveDate::parse_from_str(d, "%Y-%m-%d")
            .map_err(|_| IterError::InvalidDate(d.to_string())),
        None => Ok(now.date()),
    }
}

fn print_detail_report(
    name: &str,
    date: NaiveDate,
    records: &[Record],
    now: chrono::NaiveDateTime,
) {
    let total_minutes = merged_total_minutes(records, now, MERGE_GAP_MINUTES);
    let report = DetailReport {
        name: name.to_string(),
        date: date.format("%Y-%m-%d").to_string(),
        total_hours: round_to_half_hour(total_minutes as f64 / 60.0),
        total_hhmm: minutes_to_hhmm(total_minutes),
        messages: concat_messages(records),
    };
    print!("{}", format_detail_report(&report));
}

// ---- records: shared by manual start/stop and tmux hooks -----------------

fn start_record(db: &Db, task_id: i64) -> Result<()> {
    if db.open_record_for_task(task_id)?.is_some() {
        return Ok(()); // already has an open record; don't double-start
    }
    let record = Record {
        id: None,
        task_id,
        start: Local::now().naive_local(),
        end: None,
        message: None,
    };
    Repository::<Record>::insert(db, &record)?;
    Ok(())
}

fn close_open_record(db: &Db, task_id: i64, message: Option<&str>) -> Result<()> {
    if let Some(mut open) = db.open_record_for_task(task_id)? {
        debug_assert!(
            open.is_ongoing(),
            "open_record_for_task returned a closed record"
        );
        open.end = Some(Local::now().naive_local());
        if let Some(m) = message {
            open.message = Some(m.to_string());
        }
        Repository::<Record>::update(db, open.id.expect(ID_INVARIANT), &open)?;
    }
    Ok(())
}

/// Tears down a task's session: kills its tmux session (if any), removes
/// its worktree (if any), and deletes the session row. Called from
/// `task done`, `task delete` and `project delete`.
fn teardown_session(db: &Db, project: &Project, session: &Session) {
    if let Some(name) = &session.tmux_session_name {
        tmux::kill_session(name);
    }
    if let Some(worktree) = &session.worktree_path
        && let Err(e) = git::remove_worktree(&project.base_path, worktree)
    {
        eprintln!("warning: {e}");
    }
    if let Some(id) = session.id
        && let Err(e) = Repository::<Session>::delete(db, id)
    {
        eprintln!("warning: failed to delete session row: {e}");
    }
}

// ---- project ---------------------------------------------------------

fn dispatch_project(action: &ProjectCommand) -> Result<()> {
    match action {
        ProjectCommand::New => project_new(),
        ProjectCommand::Edit { name } => project_edit(name.as_deref()),
        ProjectCommand::Delete { name } => project_delete(name.as_deref()),
        ProjectCommand::Info { name, date } => project_info(name.as_deref(), date.as_deref()),
        ProjectCommand::List => project_list(),
    }
}

fn project_new() -> Result<()> {
    let template = Project::template();
    match yaml_edit::edit_in_nvim(&template)? {
        Some(project) => {
            if project.name.trim().is_empty() {
                return Err(IterError::EmptyProjectName);
            }
            let db = open_db();
            Repository::<Project>::insert(&db, &project)?;
            println!("created project '{}'", project.name);
        }
        None => println!("no changes -- project not created"),
    }
    Ok(())
}

fn project_edit(name: Option<&str>) -> Result<()> {
    let db = open_db();
    let existing = resolve_project_or_current(&db, name)?;
    let id = existing.id.expect(ID_INVARIANT);
    match yaml_edit::edit_in_nvim(&existing)? {
        Some(project) => {
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
    for task in db.tasks_for_project(project.id.expect(ID_INVARIANT))? {
        if let Some(session) = db.find_session_by_task(task.id.expect(ID_INVARIANT))? {
            teardown_session(&db, &project, &session);
        }
    }
    Repository::<Project>::delete(&db, project.id.expect(ID_INVARIANT))?;
    println!("deleted project '{}'", project.name);
    Ok(())
}

fn project_info(name: Option<&str>, date_filter: Option<&str>) -> Result<()> {
    let db = open_db();
    let project = resolve_project_or_current(&db, name)?;
    let now = Local::now().naive_local();
    let date = parse_date_filter(date_filter, now)?;

    // The union of every task's records that day -- this is what makes
    // working two of the project's tasks in parallel not double-count.
    let records: Vec<Record> = db
        .records_for_project(project.id.expect(ID_INVARIANT))?
        .into_iter()
        .filter(|r| r.start.date() == date)
        .collect();

    print_detail_report(&project.name, date, &records, now);
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

fn dispatch_task(action: &TaskCommand) -> Result<()> {
    match action {
        TaskCommand::New { project, issue } => task_new(project.as_deref(), *issue),
        TaskCommand::Edit { task } => task_edit(task.as_deref()),
        TaskCommand::Delete { task } => task_delete(task.as_deref()),
        TaskCommand::Info { task, date } => task_info(task.as_deref(), date.as_deref()),
        TaskCommand::List { project, status } => task_list(project.as_deref(), status.as_deref()),
        TaskCommand::Done { task } => task_done(task.as_deref()),
        TaskCommand::Start { task } => task_start(task.as_deref()),
        TaskCommand::Stop { task, message } => task_stop(task.as_deref(), message.as_deref()),
        TaskCommand::Weekday { task } => task_weekday(task.as_deref()),
    }
}

fn task_new(project_name: Option<&str>, issue: Option<i64>) -> Result<()> {
    let db = open_db();
    let project = resolve_project_or_current(&db, project_name)?;
    let project_id = project.id.expect(ID_INVARIANT);

    let mut template = Task::template(project_id);
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
    let display = format!("{}/{}", project.name, task.name);
    if let Some(session) = db.find_session_by_task(task.id.expect(ID_INVARIANT))? {
        teardown_session(&db, &project, &session);
    }
    Repository::<Task>::delete(&db, task.id.expect(ID_INVARIANT))?;
    println!("deleted task '{display}'");
    Ok(())
}

fn task_info(task_ref: Option<&str>, date_filter: Option<&str>) -> Result<()> {
    let db = open_db();
    let (project, task) = resolve_task_or_current(&db, task_ref)?;
    let now = Local::now().naive_local();
    let date = parse_date_filter(date_filter, now)?;
    let records: Vec<Record> = db
        .records_for_task(task.id.expect(ID_INVARIANT))?
        .into_iter()
        .filter(|r| r.start.date() == date)
        .collect();
    let display = format!("{}/{}", project.name, task.name);
    print_detail_report(&display, date, &records, now);
    Ok(())
}

fn task_list(project_filter: Option<&str>, status_filter: Option<&str>) -> Result<()> {
    let db = open_db();
    let status = status_filter
        .map(|s| TaskStatus::parse(s).ok_or_else(|| IterError::InvalidStatus(s.to_string())))
        .transpose()?;

    let projects: Vec<Project> = Repository::<Project>::list(&db)?
        .into_iter()
        .filter(|p| project_filter.map(|f| f == p.name).unwrap_or(true))
        .collect();

    let mut names = Vec::new();
    for p in projects {
        let tasks = db.tasks_for_project(p.id.expect(ID_INVARIANT))?;
        for t in tasks {
            if status.map(|s| s == t.status).unwrap_or(true) {
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

    close_open_record(&db, task_id, None)?;

    if let Some(session) = db.find_session_by_task(task_id)? {
        teardown_session(&db, &project, &session);
    }

    task.status = TaskStatus::Done;
    Repository::<Task>::update(&db, task_id, &task)?;
    println!("task '{display}' marked done");
    Ok(())
}

fn task_start(task_ref: Option<&str>) -> Result<()> {
    let db = open_db();
    let (project, task) = resolve_task_or_current(&db, task_ref)?;
    start_record(&db, task.id.expect(ID_INVARIANT))?;
    println!("started a record for '{}/{}'", project.name, task.name);
    Ok(())
}

fn task_stop(task_ref: Option<&str>, message: Option<&str>) -> Result<()> {
    let db = open_db();
    let (project, task) = resolve_task_or_current(&db, task_ref)?;
    close_open_record(&db, task.id.expect(ID_INVARIANT), message)?;
    println!("stopped the record for '{}/{}'", project.name, task.name);
    Ok(())
}

fn task_weekday(task_ref: Option<&str>) -> Result<()> {
    let db = open_db();
    let (project, task) = resolve_task_or_current(&db, task_ref)?;
    let now = Local::now().naive_local();
    let records = db.records_for_task(task.id.expect(ID_INVARIANT))?;
    let report = WeekdayReport {
        name: format!("{}/{}", project.name, task.name),
        weekdays: weekday_averages(&records, now),
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
    }
}

fn session_new(task_ref: &str, branch_override: Option<&str>, no_branch: bool) -> Result<()> {
    let db = open_db();
    let (project, mut task) = resolve_task(&db, task_ref)?;
    let task_id = task.id.expect(ID_INVARIANT);

    if db.find_session_by_task(task_id)?.is_some() {
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
        let branch = branch_override
            .map(|b| b.to_string())
            .unwrap_or_else(|| git::branch_name(&project.branch_template, &task.name));
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
        println!(
            "session '{tmux_session_name}' started (attach with: tmux attach -t '{tmux_session_name}')"
        );
        Some(tmux_session_name)
    } else {
        println!(
            "session started for '{task_ref}' at {cwd} (tmux disabled for this project -- use `iter task start`/`iter task stop`)"
        );
        None
    };

    let session = Session {
        id: None,
        task_id,
        tmux_session_name: final_tmux_name,
        github_branch,
        worktree_path,
    };
    Repository::<Session>::insert(&db, &session)?;

    task.status = TaskStatus::Wip;
    Repository::<Task>::update(&db, task_id, &task)?;

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
    let Some(session) = db.find_session_by_tmux_name(tmux_session)? else {
        return Ok(()); // not one of ours -- ignore
    };
    match event {
        "client-attached" => start_record(&db, session.task_id),
        "client-detached" | "session-closed" => close_open_record(&db, session.task_id, None),
        _ => Ok(()),
    }
}
