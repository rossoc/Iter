//! The shape every edit page shares: `GET` loads a row and what its form shows; `POST` applies the form, saves (or creates) and redirects, or shows the form again with the error.

use super::forms::NAME;
use super::open_db;
use super::ui::form_error::FormError;
use crate::db::{Db, Table};
use topcoat::router::error::{RouterErrorExt, see_other};

pub(super) trait EditForm {
    type Row: Table + Clone;

    fn saved(id: i64) -> String;

    fn apply(&self, row: Self::Row) -> (Self::Row, crate::error::Result<()>);

    /// The field an error is about (marked invalid, pointing at the error notice); defaults to the name field.
    fn field(error: &crate::error::IterError) -> Option<&'static str> {
        name_field(error)
    }

    fn refusal(error: &crate::error::IterError) -> FormError {
        FormError::new(error, Self::field(error))
    }

    fn save(&self, db: &Db, id: i64, row: &Self::Row) -> crate::error::Result<()> {
        db.update(id, row)
    }

    fn insert(&self, db: &Db, row: &Self::Row) -> crate::error::Result<i64> {
        db.insert(row)
    }
}

pub(super) fn name_field(error: &crate::error::IterError) -> Option<&'static str> {
    use crate::error::IterError::{CommaInName, EmptyName, NameTaken, SlashInName};
    match error {
        EmptyName(_) | NameTaken { .. } | CommaInName { .. } | SlashInName { .. } => Some(NAME),
        _ => None,
    }
}

pub(super) struct Loaded<R, X> {
    pub row: R,
    pub extra: X,
}

/// `row` holds what was typed (maybe empty); `stored` is the row as stored, for the breadcrumb and title.
pub(super) struct Refused<R, X> {
    pub row: R,
    pub stored: R,
    pub extra: X,
    pub error: FormError,
}

pub(super) fn load<R: Table, X>(
    id: i64,
    extra: impl FnOnce(&Db, &R) -> topcoat::Result<X>,
) -> topcoat::Result<Loaded<R, X>> {
    let db = open_db()?;
    let row = db.get::<R>(id)?.ok_or_not_found()?;
    Ok(Loaded {
        extra: extra(&db, &row)?,
        row,
    })
}

/// [`load`] for a form whose choices include its own table's rows: read once, row `id` taken out (a 404 when missing).
pub(super) fn load_among<R: Table, X>(
    id: i64,
    extra: impl FnOnce(&Db, &[R]) -> topcoat::Result<X>,
) -> topcoat::Result<Loaded<R, X>> {
    let db = open_db()?;
    let mut rows = db.list::<R>()?;
    let at = rows.iter().position(|r| r.id() == id).ok_or_not_found()?;
    let row = rows.swap_remove(at);
    Ok(Loaded {
        extra: extra(&db, &rows)?,
        row,
    })
}

/// A submission that doesn't validate or save comes back as the page to show again.
pub(super) fn submit<F: EditForm, X>(
    id: i64,
    form: &F,
    extra: impl FnOnce(&Db, &F::Row) -> topcoat::Result<X>,
) -> topcoat::Result<Refused<F::Row, X>> {
    let db = open_db()?;
    let existing = db.get::<F::Row>(id)?.ok_or_not_found()?;
    settle(
        &db,
        form,
        existing,
        extra,
        |db, row| {
            form.save(db, id, row)?;
            Ok(id)
        },
        F::saved,
    )
}

/// The create twin of [`submit`].
pub(super) fn create_to<F: EditForm, S, X>(
    form: &F,
    start: impl FnOnce(&Db) -> topcoat::Result<(F::Row, S)>,
    extra: impl FnOnce(&Db, S) -> topcoat::Result<X>,
    land: impl FnOnce(i64) -> String,
) -> topcoat::Result<Refused<F::Row, X>> {
    let db = open_db()?;
    let (template, seed) = start(&db)?;
    settle(
        &db,
        form,
        template,
        |db, _| extra(db, seed),
        |db, row| form.insert(db, row),
        land,
    )
}

fn settle<F: EditForm, X>(
    db: &Db,
    form: &F,
    stored: F::Row,
    extra: impl FnOnce(&Db, &F::Row) -> topcoat::Result<X>,
    write: impl FnOnce(&Db, &F::Row) -> crate::error::Result<i64>,
    land: impl FnOnce(i64) -> String,
) -> topcoat::Result<Refused<F::Row, X>> {
    let (row, valid) = form.apply(stored.clone());
    match valid.and_then(|()| write(db, &row)) {
        Ok(id) => Err(see_other(land(id)).into()),
        Err(error) => Ok(Refused {
            extra: extra(db, &stored)?,
            row,
            stored,
            error: F::refusal(&error),
        }),
    }
}
