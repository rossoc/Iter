//! `iter board` -- calendars that hold projects.

use crate::app::App;
use crate::args::BoardCommand;
use crate::commands::Run;
use crate::db::Table;
use crate::error::{IterError, Result};
use crate::md_edit::edited;
use crate::models::{Board, Project};
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
    print!("{}", render_info(&board, &projects));
    Ok(())
}

fn render_info(board: &Board, projects: &[Project]) -> String {
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
    out
}

fn board_list(app: &App) -> Result<()> {
    list_names::<Board>(&app.db)
}
