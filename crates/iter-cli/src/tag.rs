//! `iter tag` -- coloured labels shared by every task.

use crate::Run;
use crate::args::TagCommand;
use crate::entity::{create, update};
use crate::output::list_names;
use iter_core::app::App;
use iter_core::db::Table;
use iter_core::error::{IterError, Result};
use iter_core::models::Tag;
use iter_core::reporting::non_empty;

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
    update(&app.db, &existing, false, |tag| {
        // the board views find these two by name: they may be recoloured and re-described, not renamed
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
    if let Some(description) = non_empty(&tag.description) {
        println!("\n{description}");
    }
    Ok(())
}
