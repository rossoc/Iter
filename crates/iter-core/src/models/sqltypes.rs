//! SQLite conversions `rusqlite` lacks: `Duration`, `TaskStatus` and `NaiveDateTime` (via `datetime`).

use super::{Duration, TaskStatus};
use chrono::NaiveDateTime;
use rusqlite::Row;
use rusqlite::types::{FromSql, FromSqlResult, ToSql, ToSqlOutput, Type, Value, ValueRef};

/// How a `NaiveDateTime` is stored: fixed-width, so text order is time order.
pub(crate) const DATETIME_FMT: &str = "%Y-%m-%d %H:%M:%S";

/// Stored as whole minutes.
impl ToSql for Duration {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        self.0.to_sql()
    }
}

impl FromSql for Duration {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        i64::column_result(value).map(Duration)
    }
}

impl ToSql for TaskStatus {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        self.as_str().to_sql()
    }
}

impl FromSql for TaskStatus {
    /// An unrecognised status falls back to `Queue` rather than failing the
    /// read, so a row written by a newer build stays loadable.
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        Ok(TaskStatus::parse(value.as_str()?).unwrap_or(TaskStatus::Queue))
    }
}

/// A `NaiveDateTime` column as `DATETIME_FMT` text. (`rusqlite`'s own
/// `chrono` support would also write fractional seconds.)
pub(crate) mod datetime {
    use super::*;

    pub(crate) fn write(value: &NaiveDateTime) -> rusqlite::Result<ToSqlOutput<'static>> {
        Ok(ToSqlOutput::Owned(Value::Text(
            value.format(DATETIME_FMT).to_string(),
        )))
    }

    /// A malformed value fails the read (naming value and column) instead of panicking.
    pub(crate) fn read(row: &Row, index: usize) -> rusqlite::Result<NaiveDateTime> {
        let text = row.get::<_, String>(index)?;
        NaiveDateTime::parse_from_str(&text, DATETIME_FMT).map_err(|error| {
            let column = row.as_ref().column_name(index).unwrap_or("?");
            rusqlite::Error::FromSqlConversionFailure(
                index,
                Type::Text,
                format!("bad datetime '{text}' in column '{column}': {error}").into(),
            )
        })
    }

    pub(crate) mod opt {
        use super::*;

        pub(crate) fn write(
            value: &Option<NaiveDateTime>,
        ) -> rusqlite::Result<ToSqlOutput<'static>> {
            match value {
                Some(value) => super::write(value),
                None => Ok(ToSqlOutput::Owned(Value::Null)),
            }
        }

        pub(crate) fn read(row: &Row, index: usize) -> rusqlite::Result<Option<NaiveDateTime>> {
            match row.get_ref(index)? {
                ValueRef::Null => Ok(None),
                _ => super::read(row, index).map(Some),
            }
        }
    }
}
