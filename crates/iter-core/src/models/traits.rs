//! The traits that give a stored row its shape: its SQLite layout ([`Table`]), its name ([`Named`],
//! [`ProjectGroup`]) and the editor round trip ([`MarkdownBody`], [`Editable`]).

use super::Project;
use crate::error::{IterError, Result};
use rusqlite::Row;
use rusqlite::types::ToSqlOutput;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// A type's SQLite layout: table, writable columns, row reader and bound values. `Db`'s
/// generic CRUD derives every statement from these. Use `#[derive(Table)]`, which reads the
/// columns off the struct fields so they cannot drift.
pub trait Table: Sized {
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

/// An edited type with a free-form markdown `description`, `#[serde(skip)]`ed and placed below
/// the YAML front matter.
pub trait MarkdownBody {
    fn description(&self) -> &str;
    fn set_description(&mut self, description: String);

    /// Copies from `original` every field the buffer doesn't carry, so an edit can't lose them.
    fn carry_over(&mut self, _original: &Self) {}

    /// Reads the buffer's front matter; edit types wrapping a row plus a list override it
    /// ([`read_wrapped`]).
    fn from_front_matter(front: &str) -> serde_yaml::Result<Self>
    where
        Self: DeserializeOwned,
    {
        serde_yaml::from_str(front)
    }
}

/// Reads front matter of a row plus one list as two plain reads, not `#[serde(flatten)]`, which
/// buffers values and breaks bare scalars (`name: 2025`, `default_branch: 1.10` become numbers).
/// Each read ignores the other's key.
pub fn read_wrapped<R: DeserializeOwned>(
    front: &str,
    list: &str,
) -> serde_yaml::Result<(R, Vec<String>)> {
    let row = serde_yaml::from_str(front)?;
    let mut mapping: serde_yaml::Mapping = serde_yaml::from_str(front)?;
    let names = match mapping.remove(list) {
        Some(value) if !value.is_null() => serde_yaml::from_value(value)?,
        _ => Vec::new(),
    };
    Ok((row, names))
}

/// Everything the editor round trip needs: YAML out and in, description as markdown body.
/// Blanket implemented.
pub trait Editable: Serialize + DeserializeOwned + MarkdownBody {}

impl<T: Serialize + DeserializeOwned + MarkdownBody> Editable for T {}

/// A stored type with a database-wide unique `name`: how the CLI addresses it, what completion
/// offers and `list` prints. Implemented by `#[derive(Table)]` given a `kind`. Not implemented for
/// [`Task`]: its name is unique only within its project, so tasks are addressed by [`TaskRef`](super::TaskRef)
/// (`Db::task_labels` is their own query).
pub trait Named: Table {
    /// What messages call one of these: "board", "tag", ...
    const KIND: &'static str;

    fn name(&self) -> &str;

    /// Normalises and checks an edited item before it's written; by default only that it has a name.
    fn validate(&mut self) -> Result<()> {
        require_name(Self::KIND, self.name())
    }
}

/// A named container of projects (board or organization), through a nullable `ON DELETE SET NULL`
/// foreign key: deleting it only detaches its projects.
pub trait ProjectGroup: Named {
    const MEMBER_COLUMN: &'static str;

    const NOT_A_MEMBER: &'static str;

    const DETACHED: &'static str;

    fn group_of(project: &Project) -> Option<i64>;
}

pub fn require_name(kind: &'static str, name: &str) -> Result<()> {
    match name.trim().is_empty() {
        true => Err(IterError::EmptyName(kind)),
        false => Ok(()),
    }
}

/// Refuses a comma in names the web forms list comma-separated.
pub fn require_no_comma(kind: &'static str, name: &str) -> Result<()> {
    if name.contains(',') {
        return Err(IterError::CommaInName {
            kind,
            name: name.to_string(),
        });
    }
    Ok(())
}
