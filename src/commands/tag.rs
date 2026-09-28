//! `iter tag` -- coloured labels shared by every task.

use crate::app::App;
use crate::args::TagCommand;
use crate::commands::Run;
use crate::db::Table;
use crate::error::{IterError, Result};
use crate::models::Tag;
use crate::utils::crud::{create, update};
use crate::utils::output::list_names;

impl Run for TagCommand {
    fn run(&self, app: &App) -> Result<()> {
        match self {
            Self::New => create(&app.db, &Tag::template()),
            Self::Edit { name } => tag_edit(app, name),
            Self::Delete { name } => tag_delete(app, name),
            Self::Info { name } => tag_info(app, name),
            Self::List => list_names::<Tag>(&app.db),
        }
    }
}

fn tag_edit(app: &App, name: &str) -> Result<()> {
    let existing: Tag = app.db.resolve(name)?;
    update(&app.db, &existing, |tag| {
        // The board views find these two by name, so they may be recoloured
        // and re-described but not renamed.
        match existing.is_builtin() && tag.name.trim() != existing.name {
            true => Err(IterError::BuiltinTag(existing.name.clone())),
            false => Ok(()),
        }
    })
}

fn tag_delete(app: &App, name: &str) -> Result<()> {
    let tag: Tag = app.db.resolve(name)?;
    if tag.is_builtin() {
        return Err(IterError::BuiltinTag(tag.name));
    }
    app.db.delete::<Tag>(tag.id())?;
    println!("deleted tag '{}'", tag.name);
    Ok(())
}

fn tag_info(app: &App, name: &str) -> Result<()> {
    let tag: Tag = app.db.resolve(name)?;
    println!("{}  {}", tag.name, tag.color);
    if !tag.description.trim().is_empty() {
        println!("\n{}", tag.description.trim());
    }
    Ok(())
}
