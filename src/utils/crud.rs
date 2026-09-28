//! The `new`/`edit`/`delete` bodies every named entity shares. A command
//! only says what is particular to it -- the template to start from, a
//! check beyond [`Named::validate`] -- and the editor round trip, the write
//! and the message come from here.

use crate::db::Db;
use crate::error::Result;
use crate::md_edit::{edit_in_editor, edited, no_changes};
use crate::models::{Editable, GroupEdit, Named, Project, ProjectGroup};

/// `iter <entity> new`: opens `template`, validates what comes back and
/// inserts it.
pub(crate) fn create<T: Named + Editable>(db: &Db, template: &T) -> Result<()> {
    edited(template, T::KIND, "created", |mut item| {
        item.validate()?;
        db.insert(&item)?;
        Ok(format!("created {} '{}'", T::KIND, item.name()))
    })
}

/// `iter <entity> edit`: opens `existing`, lets `fixup` restore what the
/// buffer doesn't carry (and refuse what it mustn't change), validates and
/// writes it back over the same row.
///
/// `pending` says `existing` already differs from the stored row in a way
/// the buffer can't show -- a field set from a command-line flag, like
/// `project edit --board`. Quitting the editor without changes then still
/// saves `existing`, rather than dropping the flag as "no changes".
pub(crate) fn update<T: Named + Editable + Clone>(
    db: &Db,
    existing: &T,
    pending: bool,
    fixup: impl FnOnce(&mut T) -> Result<()>,
) -> Result<()> {
    let mut item = match edit_in_editor(existing)? {
        Some(item) => item,
        None if pending => existing.clone(),
        None => {
            println!("{}", no_changes(T::KIND, "updated"));
            return Ok(());
        }
    };
    fixup(&mut item)?;
    item.validate()?;
    db.update(existing.id(), &item)?;
    println!("updated {} '{}'", T::KIND, item.name());
    Ok(())
}

/// `iter board edit`/`iter organization edit`: edits the group and its
/// roster together -- the buffer lists the projects in it by name, and
/// saving makes that list the membership.
pub(crate) fn update_group<G: ProjectGroup + Editable>(db: &Db, existing: G) -> Result<()> {
    let id = existing.id();
    let projects = db
        .projects_in::<G>(id)?
        .into_iter()
        .map(|p| p.name)
        .collect();
    let template = GroupEdit {
        group: existing,
        projects,
    };
    edited(&template, G::KIND, "updated", |edit| {
        let mut group = edit.group;
        group.validate()?;
        // Resolved before anything is written, so an unknown name leaves
        // the group untouched.
        let project_ids = db.ids_by_name::<Project>(&edit.projects)?;
        db.update(id, &group)?;
        db.set_projects::<G>(id, &project_ids)?;
        Ok(format!("updated {} '{}'", G::KIND, group.name()))
    })
}

/// Deletes a board/organization row only. Its projects survive -- the
/// schema's `ON DELETE SET NULL` just detaches them -- and the message says
/// how many were kept.
pub(crate) fn delete_group<G: ProjectGroup>(db: &Db, group: &G) -> Result<()> {
    let id = group.id();
    let kept = db.projects_in::<G>(id)?.len();
    db.delete::<G>(id)?;
    println!("{}", deleted_message::<G>(group.name(), kept));
    Ok(())
}

fn deleted_message<G: ProjectGroup>(name: &str, kept: usize) -> String {
    let suffix = match kept {
        0 => String::new(),
        1 => format!(" -- 1 project kept, now {}", G::DETACHED),
        n => format!(" -- {n} projects kept, now {}", G::DETACHED),
    };
    format!("deleted {} '{name}'{suffix}", G::KIND)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Board, Organization};

    #[test]
    fn a_deleted_group_says_how_many_projects_it_left_behind() {
        assert_eq!(deleted_message::<Board>("work", 0), "deleted board 'work'");
        assert_eq!(
            deleted_message::<Board>("work", 1),
            "deleted board 'work' -- 1 project kept, now on no board"
        );
        assert_eq!(
            deleted_message::<Organization>("acme", 3),
            "deleted organization 'acme' -- 3 projects kept, now without an organization"
        );
    }
}
