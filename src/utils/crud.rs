//! The `new`/`edit`/`delete` bodies every named entity shares. A command
//! only says what is particular to it -- the template to start from, a
//! check beyond [`Named::validate`] -- and the editor round trip, the write
//! and the message come from here.

use crate::db::Db;
use crate::error::Result;
use crate::md_edit::edited;
use crate::models::{Editable, Named, ProjectGroup};

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
pub(crate) fn update<T: Named + Editable>(
    db: &Db,
    existing: &T,
    fixup: impl FnOnce(&mut T) -> Result<()>,
) -> Result<()> {
    let id = existing.id();
    edited(existing, T::KIND, "updated", |mut item| {
        fixup(&mut item)?;
        item.validate()?;
        db.update(id, &item)?;
        Ok(format!("updated {} '{}'", T::KIND, item.name()))
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
