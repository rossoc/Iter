//! How commands print. The `list` commands emit YAML so their output pipes
//! into anything; the `info` reports render themselves (see
//! [`crate::reporting::Report`]).

use crate::db::{Db, Table};
use crate::error::Result;
use crate::models::Named;

/// Prints `value` as YAML -- what every `list` command emits, and the one
/// place `serde_yaml` is reached for outside the report renderers.
pub(crate) fn print_yaml<T: serde::Serialize>(value: &T) -> Result<()> {
    print!("{}", serde_yaml::to_string(value)?);
    Ok(())
}

/// `iter <entity> list`: every `T`'s name as a YAML list. One command body
/// for every named entity.
pub(crate) fn list_names<T: Table + Named>(db: &Db) -> Result<()> {
    print_yaml(&db.names::<T>()?)
}
