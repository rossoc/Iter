//! `iter tag` -- coloured labels shared by every task.

use crate::app::App;
use crate::args::TagCommand;
use crate::commands::Run;
use crate::db::Table;
use crate::error::{IterError, Result};
use crate::md_edit::edited;
use crate::models::{Tag, is_hex_color};
use crate::utils::output::list_names;
use crate::utils::resolve::resolve_tag;

impl Run for TagCommand {
    fn run(&self, app: &App) -> Result<()> {
        match self {
            Self::New => tag_new(app),
            Self::Edit { name } => tag_edit(app, name),
            Self::Delete { name } => tag_delete(app, name),
            Self::Info { name } => tag_info(app, name),
            Self::List => list_names::<Tag>(&app.db),
        }
    }
}

fn check(tag: &mut Tag) -> Result<()> {
    tag.name = tag.name.trim().to_string();
    tag.color = tag.color.trim().to_lowercase();
    if tag.name.is_empty() {
        return Err(IterError::EmptyTagName);
    }
    if !is_hex_color(&tag.color) {
        return Err(IterError::InvalidTagColor(tag.color.clone()));
    }
    Ok(())
}

fn tag_new(app: &App) -> Result<()> {
    let db = &app.db;
    edited(&Tag::template(), "tag", "created", |mut tag| {
        check(&mut tag)?;
        db.insert(&tag)?;
        Ok(format!("created tag '{}'", tag.name))
    })
}

fn tag_edit(app: &App, name: &str) -> Result<()> {
    let db = &app.db;
    let existing = resolve_tag(db, name)?;
    let id = existing.id();
    edited(&existing, "tag", "updated", |mut tag| {
        check(&mut tag)?;
        // The board views find these two by name, so they may be recoloured
        // and re-described but not renamed.
        if existing.is_builtin() && tag.name != existing.name {
            return Err(IterError::BuiltinTag(existing.name.clone()));
        }
        db.update(id, &tag)?;
        Ok(format!("updated tag '{}'", tag.name))
    })
}

fn tag_delete(app: &App, name: &str) -> Result<()> {
    let tag = resolve_tag(&app.db, name)?;
    if tag.is_builtin() {
        return Err(IterError::BuiltinTag(tag.name));
    }
    app.db.delete::<Tag>(tag.id())?;
    println!("deleted tag '{}'", tag.name);
    Ok(())
}

fn tag_info(app: &App, name: &str) -> Result<()> {
    let tag = resolve_tag(&app.db, name)?;
    println!("{}  {}", tag.name, tag.color);
    if !tag.description.trim().is_empty() {
        println!("\n{}", tag.description.trim());
    }
    Ok(())
}
