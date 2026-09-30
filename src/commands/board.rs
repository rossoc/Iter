//! `iter board` -- calendars that hold projects.

use crate::app::App;
use crate::args::BoardCommand;
use crate::commands::Run;
use crate::db::Table;
use crate::error::Result;
use crate::models::{Board, Card, Priority, Project, START_TIME_FMT, TaskStatus, end_text};
use crate::reporting::non_empty;
use crate::utils::crud::{create, delete_group, update_group};
use crate::utils::output::list_names;
use crate::utils::resolve::resolve_group_or_current;

impl Run for BoardCommand {
    fn run(&self, app: &App) -> Result<()> {
        match self {
            Self::New => create(&app.db, &Board::template()),
            Self::Edit { name } => update_group(&app.db, board(app, name)?),
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
    let cards = app.db.cards(board.id())?;
    print!("{}", render_info(board, &projects, &cards));
    Ok(())
}

fn render_info(board: &Board, projects: &[Project], cards: &[Card]) -> String {
    let mut out = format!("{}\n", board.name);
    if let Some(description) = non_empty(&board.description) {
        out.push_str(&format!("\n{description}\n"));
    }
    out.push_str("\nprojects:\n");
    if projects.is_empty() {
        out.push_str("  (none)\n");
    }
    for project in projects {
        out.push_str(&format!("  {}\n", project.name));
    }
    out.push_str(&render_agenda(cards));
    out
}

/// Open tasks that have a `start_time`, soonest first, then the ones that
/// have none. Finished work is left off: an agenda is what's still ahead.
fn render_agenda(cards: &[Card]) -> String {
    let open = cards.iter().filter(|c| c.task.status != TaskStatus::Done);
    let (mut scheduled, mut unscheduled): (Vec<_>, Vec<_>) =
        open.partition(|c| c.task.start_time.is_some());
    scheduled.sort_by(|a, b| (a.task.start_time, &a.label).cmp(&(b.task.start_time, &b.label)));
    unscheduled.sort_by(|a, b| a.label.cmp(&b.label));

    let mut out = String::from("\nagenda:\n");
    if scheduled.is_empty() {
        out.push_str("  (none)\n");
    }
    for card in scheduled {
        let start = card.task.start_time.expect("partitioned on start_time");
        let mut slot = start.format(START_TIME_FMT).to_string();
        if let Some(end) = card.task.end_time() {
            slot.push_str(&format!("-{}", end_text(start, end)));
        }
        out.push_str(&format!(
            "  {slot}  {}{}\n",
            card.label,
            markers(card.priority)
        ));
    }
    if !unscheduled.is_empty() {
        out.push_str("\nunscheduled:\n");
        for card in unscheduled {
            out.push_str(&format!("  {}{}\n", card.label, markers(card.priority)));
        }
    }
    out
}

/// ` [U]`, ` [I]` or ` [UI]` for urgent and/or important tasks, else empty.
fn markers(priority: Priority) -> String {
    let flags: String = [(priority.urgent, 'U'), (priority.important, 'I')]
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
    use crate::models::{Duration, Task, TaskStatus};
    use chrono::NaiveDateTime;

    fn task(name: &str, start: Option<&str>, minutes: Option<i64>) -> Card {
        let mut t = Task::template(1, String::new(), TaskStatus::Queue);
        t.name = name.to_string();
        t.start_time =
            start.map(|s| NaiveDateTime::parse_from_str(s, START_TIME_FMT).expect("valid"));
        t.duration = minutes.map(Duration);
        Card {
            label: format!("p/{name}"),
            task: t,
            priority: Priority::default(),
        }
    }

    #[test]
    fn agenda_orders_by_start_and_lists_unscheduled_last() {
        let mut late = task("late", Some("2026-09-29 10:00"), Some(90));
        late.priority = Priority {
            urgent: true,
            important: true,
        };
        let mut done = task("done", Some("2026-09-27 08:00"), None);
        done.task.status = TaskStatus::Done;
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
