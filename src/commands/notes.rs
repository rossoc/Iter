//! `iter notes` -- the daily notes of a project or an organization, from the
//! command line (the web has them under the Notes tab).

use crate::app::App;
use crate::args::{NotesCommand, NotesScope};
use crate::commands::Run;
use crate::error::{IterError, Result};
use crate::models::Organization;
use crate::notes;
use crate::utils::resolve::resolve_project_or_current;
use std::path::PathBuf;

fn dir(app: &App, scope: &NotesScope) -> Result<PathBuf> {
    let db = &app.db;
    let found = match &scope.org {
        Some(name) => notes::org_dir(&db.resolve::<Organization>(name)?),
        None => {
            let project = resolve_project_or_current(db, scope.project.as_deref())?;
            let org = match project.organization_id {
                Some(id) => db.get::<Organization>(id)?,
                None => None,
            };
            notes::project_dir(&project, &org)
        }
    };
    found.ok_or_else(|| {
        IterError::CommandFailed(
            "no notes folder: set `notes_path` on it or its organization".into(),
        )
    })
}

fn day(text: &str) -> Result<chrono::NaiveDate> {
    notes::parse_date(text)
        .ok_or_else(|| IterError::CommandFailed(format!("`{text}` is not a day (YYYY-MM-DD)")))
}

impl Run for NotesCommand {
    fn run(&self, app: &App) -> Result<()> {
        let today = app.now.date();
        match self {
            Self::Today { scope } => {
                let dir = dir(app, scope)?;
                notes::create(&dir, today)?;
                println!(
                    "{}",
                    dir.join(format!("{}.md", today.format("%Y-%m-%d")))
                        .display()
                );
                Ok(())
            }
            Self::List { scope } => {
                for note in notes::list(&dir(app, scope)?) {
                    println!("{}  {}", note.date.format("%Y-%m-%d"), note.title);
                }
                Ok(())
            }
            Self::Show { date, scope } => {
                let date = match date {
                    Some(d) => day(d)?,
                    None => today,
                };
                let note = notes::get(&dir(app, scope)?, date)
                    .ok_or_else(|| IterError::CommandFailed(format!("no note of {date}")))?;
                println!("{}", note.body);
                Ok(())
            }
        }
    }
}
