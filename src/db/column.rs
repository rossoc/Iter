//! How one field crosses the SQLite boundary.

use super::*;

/// How one field crosses the SQLite boundary. This is where the per-type
/// conversions live -- a `bool` stored as 0/1, a `NaiveDateTime` as
/// formatted text -- so that `#[derive(Table)]` can generate the same two
/// lines for every field and let inference pick the right conversion.
pub(crate) trait Column: Sized {
    /// Reads this field from column `index` of `row`.
    fn from_sql(row: &Row, index: usize) -> rusqlite::Result<Self>;

    /// Binds this field for writing.
    fn to_sql(&self) -> Value;
}

impl Column for i64 {
    fn from_sql(row: &Row, index: usize) -> rusqlite::Result<Self> {
        row.get(index)
    }

    fn to_sql(&self) -> Value {
        Value::Integer(*self)
    }
}

impl Column for String {
    fn from_sql(row: &Row, index: usize) -> rusqlite::Result<Self> {
        row.get(index)
    }

    fn to_sql(&self) -> Value {
        Value::Text(self.clone())
    }
}

impl Column for bool {
    fn from_sql(row: &Row, index: usize) -> rusqlite::Result<Self> {
        Ok(row.get::<_, i64>(index)? != 0)
    }

    fn to_sql(&self) -> Value {
        Value::Integer(*self as i64)
    }
}

impl Column for NaiveDateTime {
    /// A malformed value fails the read rather than panicking, so one bad
    /// row is an error page, not a crashed request. The message names the
    /// value and its column, since that is all the person has to go on.
    fn from_sql(row: &Row, index: usize) -> rusqlite::Result<Self> {
        let text = row.get::<_, String>(index)?;
        str_to_dt(&text).map_err(|error| {
            let column = row.as_ref().column_name(index).unwrap_or("?");
            rusqlite::Error::FromSqlConversionFailure(
                index,
                Type::Text,
                format!("bad datetime '{text}' in column '{column}': {error}").into(),
            )
        })
    }

    fn to_sql(&self) -> Value {
        Value::Text(dt_to_str(*self))
    }
}

/// Stored as whole minutes.
impl Column for Duration {
    fn from_sql(row: &Row, index: usize) -> rusqlite::Result<Self> {
        Ok(Duration(row.get(index)?))
    }

    fn to_sql(&self) -> Value {
        Value::Integer(self.0)
    }
}

impl Column for TaskStatus {
    /// An unrecognised status falls back to `Queue` rather than failing the
    /// read, so a row written by a newer build stays loadable.
    fn from_sql(row: &Row, index: usize) -> rusqlite::Result<Self> {
        Ok(TaskStatus::parse(&row.get::<_, String>(index)?).unwrap_or(TaskStatus::Queue))
    }

    fn to_sql(&self) -> Value {
        Value::Text(self.as_str().to_string())
    }
}

/// A nullable column: SQL `NULL` on one side, `None` on the other. Lets
/// every optional field reuse its inner type's conversion.
impl<T: Column> Column for Option<T> {
    fn from_sql(row: &Row, index: usize) -> rusqlite::Result<Self> {
        match row.get_ref(index)? {
            ValueRef::Null => Ok(None),
            _ => T::from_sql(row, index).map(Some),
        }
    }

    fn to_sql(&self) -> Value {
        match self {
            Some(value) => value.to_sql(),
            None => Value::Null,
        }
    }
}
