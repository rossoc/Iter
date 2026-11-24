//! `iter instruction`: the instructions an agent can be given, and where each comes from, read
//! through the same lookup as `iter task prompt` and the web (`agent::instruction`).

use crate::Run;
use crate::args::InstructionCommand;
use iter_core::agent::instruction::{self, Layers, Source};
use iter_core::app::App;
use iter_core::error::{IterError, Result};
use iter_core::locate::{project_or_current, task_or_current};

impl Run for InstructionCommand {
    fn run(&self, app: &App) -> Result<()> {
        match self {
            Self::List { task } => {
                let found = instruction::list(&layers(app, task.as_deref())?);
                let width = found.iter().map(|i| i.name.len()).max().unwrap_or(0);
                for i in found {
                    println!(
                        "{:width$}  {:7}  {}",
                        i.name,
                        origin(i.source),
                        i.front.description
                    );
                }
                Ok(())
            }
            Self::Show { name, task } => {
                let found = instruction::find(name, &layers(app, task.as_deref())?)
                    .ok_or_else(|| IterError::CommandFailed(format!("no instruction '{name}'")))?;
                print!("{}", found.raw);
                Ok(())
            }
        }
    }
}

/// Where an instruction comes from, in one word.
fn origin(source: Source) -> &'static str {
    match source {
        Source::Task => "task",
        Source::Project => "project",
        Source::Organization => "org",
        Source::Global => "config",
    }
}

/// The layers looked in: the task's (named, else the current one), else the current project's,
/// else only iter's config folder.
fn layers(app: &App, task: Option<&str>) -> Result<Layers> {
    let db = &app.db;
    match task_or_current(db, task) {
        Ok((project, task)) => return instruction::layers_for_task(db, &project, &task),
        Err(e) if task.is_some() => return Err(e),
        Err(_) => {}
    }
    if let Ok(project) = project_or_current(db, None) {
        return instruction::layers_of(db, &project);
    }
    Ok(instruction::global()
        .map(|dir| vec![(Source::Global, dir)])
        .unwrap_or_default())
}
