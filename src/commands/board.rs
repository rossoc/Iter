//! `iter board` -- calendars that hold projects.

use crate::app::App;
use crate::args::BoardCommand;
use crate::commands::Run;
use crate::db::Table;
use crate::error::Result;
use crate::models::{Board, Project, START_TIME_FMT, Task, TaskStatus, task_ref};
use crate::utils::crud::{create, delete_group, update};
use crate::utils::output::list_names;
use crate::utils::resolve::resolve_group_or_current;

impl Run for BoardCommand {
    fn run(&self, app: &App) -> Result<()> {
        match self {
            Self::New => create(&app.db, &Board::template()),
            Self::Edit { name } => update(&app.db, &board(app, name)?, |_| Ok(())),
            Self::Delete { name } => delete_group(&app.db, &board(app, name)?),
            Self::Info { name } => board_info(app, &board(app, name)?),
            Self::List => list_names::<Board>(&app.db),
        }
    }
}

/// An explicit board name, or -- when none is given -- the board of the
/// tmux session's project.
fn board(app: &App, name: &Option<String>) -> Result<Board> {
    resolve_group_or_current(&app.db, name.as_deref())
}

fn board_info(app: &App, board: &Board) -> Result<()> {
    let projects = app.db.projects_in::<Board>(board.id())?;
    // Tasks are flattened across projects and named `<project>/<task>`: the
    // agenda is about *when*, so it isn't grouped by project.
    let mut tasks = Vec::new();
    for project in &projects {
        for task in app.db.tasks_for_project(project.id())? {
            tasks.push((task_ref(&project.name, &task.name), task));
        }
    }
    print!("{}", render_info(board, &projects, &tasks));
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
