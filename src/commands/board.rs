//! `iter board` -- calendars that hold projects.

use crate::app::App;
use crate::args::BoardCommand;
use crate::commands::Run;
use crate::db::Table;
use crate::error::{IterError, Result};
use crate::md_edit::edited;
use crate::models::{Board, Project, START_TIME_FMT, Task, TaskStatus, task_ref};
use crate::utils::output::list_names;
use crate::utils::resolve::{current_session_task, resolve_board};

impl Run for BoardCommand {
    fn run(&self, app: &App) -> Result<()> {
        match self {
            Self::New => board_new(app),
            Self::Edit { name } => board_edit(app, name.as_deref()),
            Self::Delete { name } => board_delete(app, name.as_deref()),
            Self::Info { name } => board_info(app, name.as_deref()),
            Self::List => board_list(app),
        }
    }
}

/// An explicit board name, or -- when none is given -- the board of the
/// tmux session's project. Erroring when that project is on no board is
/// deliberate: binding is optional, so there's no "current" board to fall
/// back to.
fn resolve_board_or_current(app: &App, name: Option<&str>) -> Result<Board> {
    let db = &app.db;
    if let Some(name) = name {
        return resolve_board(db, name);
    }
    let project = current_session_task(db)?.0;
    let id = project
        .board_id
        .ok_or_else(|| IterError::ProjectHasNoBoard(project.name.clone()))?;
    db.get::<Board>(id)?.ok_or(IterError::OrphanProjectBoard)
}

fn board_new(app: &App) -> Result<()> {
    let db = &app.db;
    edited(&Board::template(), "board", "created", |board| {
        if board.name.trim().is_empty() {
            return Err(IterError::EmptyBoardName);
        }
        db.insert(&board)?;
        Ok(format!("created board '{}'", board.name))
    })
}

fn board_edit(app: &App, name: Option<&str>) -> Result<()> {
    let db = &app.db;
    let existing = resolve_board_or_current(app, name)?;
    let id = existing.id();
    edited(&existing, "board", "updated", |board| {
        if board.name.trim().is_empty() {
            return Err(IterError::EmptyBoardName);
        }
        db.update(id, &board)?;
        Ok(format!("updated board '{}'", board.name))
    })
}

/// Deletes the board row only. Its projects survive -- the schema's
/// `ON DELETE SET NULL` just clears their `board_id`.
fn board_delete(app: &App, name: Option<&str>) -> Result<()> {
    let db = &app.db;
    let board = resolve_board_or_current(app, name)?;
    let id = board.id();
    let kept = db.projects_for_board(id)?.len();

    db.delete::<Board>(id)?;

    let unbound = match kept {
        0 => String::new(),
        1 => " -- 1 project kept, now on no board".to_string(),
        n => format!(" -- {n} projects kept, now on no board"),
    };
    println!("deleted board '{}'{unbound}", board.name);
    Ok(())
}

fn board_info(app: &App, name: Option<&str>) -> Result<()> {
    let board = resolve_board_or_current(app, name)?;
    let projects = app.db.projects_for_board(board.id())?;
    // Tasks are flattened across projects and named `<project>/<task>`: the
    // agenda is about *when*, so it isn't grouped by project.
    let mut tasks = Vec::new();
    for project in &projects {
        for task in app.db.tasks_for_project(project.id())? {
            tasks.push((task_ref(&project.name, &task.name), task));
        }
    }
    print!("{}", render_info(&board, &projects, &tasks));
    Ok(())
}

fn render_info(board: &Board, projects: &[Project], tasks: &[(String, Task)]) -> String {
    let mut out = format!("{}\n", board.name);
    if !board.description.trim().is_empty() {
        out.push_str(&format!("\n{}\n", board.description.trim()));
    }
    out.push_str("\nprojects:\n");
    if projects.is_empty() {
        out.push_str("  (none)\n");
    }
    for project in projects {
        out.push_str(&format!("  {}\n", project.name));
    }
    out.push_str(&render_agenda(tasks));
    out
}

/// Open tasks that have a `start_time`, soonest first, then the ones that
/// have none. Finished work is left off: an agenda is what's still ahead.
fn render_agenda(tasks: &[(String, Task)]) -> String {
    let open = tasks.iter().filter(|(_, t)| t.status != TaskStatus::Done);
    let (mut scheduled, mut unscheduled): (Vec<_>, Vec<_>) =
        open.partition(|(_, t)| t.start_time.is_some());
    scheduled.sort_by_key(|(name, t)| (t.start_time, name.clone()));
    unscheduled.sort_by_key(|(name, _)| name.clone());

    let mut out = String::from("\nagenda:\n");
    if scheduled.is_empty() {
        out.push_str("  (none)\n");
    }
    for (name, task) in scheduled {
        let start = task.start_time.expect("partitioned on start_time");
        let mut slot = start.format(START_TIME_FMT).to_string();
        if let Some(duration) = task.duration {
            let end = start + chrono::Duration::minutes(duration.minutes());
            let end_fmt = if end.date() == start.date() {
                "%H:%M"
            } else {
                START_TIME_FMT
            };
            slot.push_str(&format!("-{}", end.format(end_fmt)));
        }
        out.push_str(&format!("  {slot}  {name}{}\n", markers(task)));
    }
    if !unscheduled.is_empty() {
        out.push_str("\nunscheduled:\n");
        for (name, task) in unscheduled {
            out.push_str(&format!("  {name}{}\n", markers(task)));
        }
    }
    out
}

/// ` [U]`, ` [I]` or ` [UI]` for urgent and/or important tasks, else empty.
fn markers(task: &Task) -> String {
    let flags: String = [(task.urgency, 'U'), (task.importance, 'I')]
        .iter()
        .filter(|(on, _)| *on)
        .map(|(_, c)| *c)
        .collect();
    if flags.is_empty() {
        flags
    } else {
        format!(" [{flags}]")
    }
}

fn board_list(app: &App) -> Result<()> {
    list_names::<Board>(&app.db)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Duration, TaskStatus};
    use chrono::NaiveDateTime;

    fn task(name: &str, start: Option<&str>, minutes: Option<i64>) -> (String, Task) {
        let mut t = Task::template(1, String::new(), TaskStatus::Queue);
        t.name = name.to_string();
        t.start_time =
            start.map(|s| NaiveDateTime::parse_from_str(s, START_TIME_FMT).expect("valid"));
        t.duration = minutes.map(Duration);
        (format!("p/{name}"), t)
    }

    #[test]
    fn agenda_orders_by_start_and_lists_unscheduled_last() {
        let mut late = task("late", Some("2026-09-29 10:00"), Some(90));
        late.1.urgency = true;
        late.1.importance = true;
        let mut done = task("done", Some("2026-09-27 08:00"), None);
        done.1.status = TaskStatus::Done;
        let out = render_agenda(&[
            late,
            task("early", Some("2026-09-28 23:30"), Some(60)),
            task("floating", None, None),
            done,
        ]);
        assert_eq!(
            out,
            "\nagenda:\n  2026-09-28 23:30-2026-09-29 00:30  p/early\n  \
             2026-09-29 10:00-11:30  p/late [UI]\n\nunscheduled:\n  p/floating\n"
        );
    }
}
