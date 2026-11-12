//! The storage layout `#[derive(Table)]` implements; `db` builds its generic CRUD on it.

use rusqlite::Row;
use rusqlite::types::ToSqlOutput;

/// A type's SQLite layout: table, writable columns, row reader and bound values. `Db`'s
/// generic CRUD derives every statement from these. Use `#[derive(Table)]`, which reads the
/// columns off the struct fields so they cannot drift.
pub(crate) trait Table: Sized {
    const NAME: &'static str;

    /// Writable columns in `values` order; `id` is absent (assigned on insert, key on update).
    const COLUMNS: &'static [&'static str];

    /// Trailing clause for a bare `Db::list`, e.g. `"ORDER BY name"`.
    const LIST_TAIL: &'static str = "";

    /// Reads a row shaped `id, {COLUMNS}` -- the shape `select_sql` builds.
    fn from_row(row: &Row) -> rusqlite::Result<Self>;

    fn values(&self) -> rusqlite::Result<Vec<ToSqlOutput<'_>>>;

    /// `None` until inserted.
    fn row_id(&self) -> Option<i64>;

    /// The id of a row read from the database; panics otherwise, so the invariant is stated once.
    fn id(&self) -> i64 {
        self.row_id()
            .expect("a row loaded from the database always has an id")
    }
}
