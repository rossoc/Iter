//! How commands print. The `list` commands emit YAML so their output pipes
//! into anything; the `info` reports render themselves (see
//! [`crate::reporting::Report`]); the commands an agent calls answer in the
//! [`Format`] it asks for.

use crate::db::{Db, Table};
use crate::error::{IterError, Result};
use crate::models::Named;
use serde::Serialize;

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

#[derive(clap::ValueEnum, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Format {
    /// Readable lines
    #[default]
    Text,
    /// The same answer as YAML
    Yaml,
    /// The same answer as JSON
    Json,
}

/// `value` in `format`; `text` makes the readable form, only when asked.
pub fn render<T: Serialize>(
    format: Format,
    value: &T,
    text: impl FnOnce() -> String,
) -> Result<String> {
    Ok(match format {
        Format::Text => text(),
        Format::Yaml => serde_yaml::to_string(value)?,
        Format::Json => serde_json::to_string_pretty(value)
            .map_err(|e| IterError::CommandFailed(e.to_string()))?,
    })
}

/// [`render`], printed with exactly one newline at the end.
pub fn emit<T: Serialize>(format: Format, value: &T, text: impl FnOnce() -> String) -> Result<()> {
    println!("{}", render(format, value, text)?.trim_end());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_value_in_each_format() {
        let v = serde_json::json!({ "a": 1 });
        assert_eq!(render(Format::Text, &v, || "one".into()).unwrap(), "one");
        assert_eq!(
            render(Format::Yaml, &v, || unreachable!()).unwrap(),
            "a: 1\n"
        );
        assert_eq!(
            render(Format::Json, &v, || unreachable!()).unwrap(),
            "{\n  \"a\": 1\n}"
        );
    }
}
