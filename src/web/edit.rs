//! The shape every edit page shares. `GET` loads a row plus whatever else
//! its form shows; `POST` applies the submitted form to the row, saves it
//! and redirects -- or shows the form again with the error, filled in with
//! what was typed.

use super::layout::Nav;
use super::open_db;
use crate::db::{Db, Table};
use topcoat::router::error::{RouterErrorExt, see_other};

/// A submitted edit form: how it lands on its row, and what it writes.
pub(super) trait EditForm {
    type Row: Table;

    /// Where the browser goes once row `id` is saved.
    fn saved(id: i64) -> String;

    /// `row` with the submission applied, and whether that is acceptable.
    /// The row is filled in even when it isn't, so the form can be shown
    /// again with what was typed.
    fn apply(&self, row: Self::Row) -> (Self::Row, crate::error::Result<()>);

    /// Writes `row` over `id` -- and, for a form that carries links
    /// (projects, tags), those too.
    fn save(&self, db: &Db, id: i64, row: &Self::Row) -> crate::error::Result<()> {
        db.update(id, row)
    }
}

/// What an edit page renders: the sidebar, the row, and whatever else its
/// form shows beside the row.
pub(super) struct Loaded<R, X> {
    pub nav: Nav,
    pub row: R,
    pub extra: X,
}

/// Loads row `id`, and what `extra` reads beside it, over one connection.
pub(super) fn load<R: Table, X>(
    id: i64,
    extra: impl FnOnce(&Db) -> crate::error::Result<X>,
) -> topcoat::Result<Loaded<R, X>> {
    let db = open_db()?;
    let row = db.get::<R>(id)?.ok_or_not_found()?;
    Ok(Loaded {
        extra: extra(&db)?,
        nav: Nav::load(&db)?,
        row,
    })
}

/// Applies `form` to row `id` and saves it, redirecting to
/// [`EditForm::saved`]. A submission that doesn't validate or save comes
/// back as the page to show again, and the error to show on it.
pub(super) fn submit<F: EditForm, X>(
    id: i64,
    form: &F,
    extra: impl FnOnce(&Db) -> crate::error::Result<X>,
) -> topcoat::Result<(Loaded<F::Row, X>, String)> {
    let db = open_db()?;
    let existing = db.get::<F::Row>(id)?.ok_or_not_found()?;
    let (row, valid) = form.apply(existing);
    match valid.and_then(|()| form.save(&db, id, &row)) {
        Ok(()) => Err(see_other(F::saved(id)).into()),
        Err(error) => Ok((
            Loaded {
                extra: extra(&db)?,
                nav: Nav::load(&db)?,
                row,
            },
            error.to_string(),
        )),
    }
}
