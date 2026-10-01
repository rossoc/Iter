//! The shape every edit page shares. `GET` loads a row plus whatever else
//! its form shows; `POST` applies the submitted form to the row, saves it
//! (or, for a new one, inserts it: `create_to`) and redirects -- or shows the form again with the error, filled in with
//! what was typed.

use super::forms::NAME;
use super::open_db;
use super::ui::form_error::FormError;
use crate::db::{Db, Table};
use topcoat::router::error::{RouterErrorExt, see_other};

/// A submitted edit form: how it lands on its row, and what it writes.
pub(super) trait EditForm {
    type Row: Table + Clone;

    /// Where the browser goes once row `id` is saved.
    fn saved(id: i64) -> String;

    /// `row` with the submission applied, and whether that is acceptable.
    /// The row is filled in even when it isn't, so the form can be shown
    /// again with what was typed.
    fn apply(&self, row: Self::Row) -> (Self::Row, crate::error::Result<()>);

    /// The form field an error is about, if it is about one: that field is
    /// marked invalid and points at the error notice. By default the name
    /// ones ([`name_field`]); a form overrides it for its extras, falling
    /// back on that. See [`Self::refusal`].
    fn field(error: &crate::error::IterError) -> Option<&'static str> {
        name_field(error)
    }

    /// The refusal to show for `error`: its message, and the field it is
    /// about ([`Self::field`]).
    fn refusal(error: &crate::error::IterError) -> FormError {
        FormError::new(error, Self::field(error))
    }

    /// Writes `row` over `id` -- and, for a form that carries links
    /// (projects, tags), those too.
    fn save(&self, db: &Db, id: i64, row: &Self::Row) -> crate::error::Result<()> {
        db.update(id, row)
    }

    /// Writes `row` as a new one, and returns its id: the create twin of
    /// [`Self::save`].
    fn insert(&self, db: &Db, row: &Self::Row) -> crate::error::Result<i64> {
        db.insert(row)
    }
}

/// [`NAME`] when `error` is about the name every edit form has: empty, taken
/// or with a comma in it.
pub(super) fn name_field(error: &crate::error::IterError) -> Option<&'static str> {
    use crate::error::IterError::{CommaInName, EmptyName, NameTaken};
    match error {
        EmptyName(_) | NameTaken { .. } | CommaInName { .. } => Some(NAME),
        _ => None,
    }
}

/// What an edit page renders: the row, and whatever else its form shows
/// beside it (choices, links).
pub(super) struct Loaded<R, X> {
    pub row: R,
    pub extra: X,
}

/// A submission that was refused: the row with what was typed, the error,
/// and the row as stored (`stored`: for a breadcrumb or title -- `row` holds
/// what was typed, which may be empty).
pub(super) struct Refused<R, X> {
    pub row: R,
    pub stored: R,
    pub extra: X,
    pub error: FormError,
}

/// Loads row `id`, and what `extra` reads beside it (given the row), over one
/// connection.
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

/// [`load`] for a form whose choices include the rows of its own table (a
/// board's edit form lists the boards for the "currently in" names): the rows
/// are read once, row `id` is taken out of them (a 404 when it is not there:
/// no read by key of its own), and `extra` gets the others.
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

/// Applies `form` to row `id` and saves it, redirecting to
/// [`EditForm::saved`]. A submission that doesn't validate or save comes
/// back as the page to show again, and the error to show on it. `extra`
/// runs only then, given the stored row.
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

/// The create twin of [`submit`]: `start` gives the row the form starts from
/// (the template) and what it read to build it (`S`), over the request's one
/// connection. The form is applied and written by the same hooks, and the
/// browser goes where `land` says, given the new id (a form that adds a row
/// from a list goes back to it). A refusal comes back with the template as
/// `stored`; `extra` runs only then, given `S`.
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

/// The path both share: `form` applied to `stored`, then `write` (which
/// returns the id the row now has). Done: redirect. Refused: what to show
/// again.
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
