use crate::config::config;
use crate::error::{IterError, Result};
#[cfg(feature = "web")]
use crate::models::Organization;
use crate::models::{
    Board, Card, Duration, IMPORTANT_TAG, Named, Priority, Project, ProjectGroup, Push, Session,
    SessionConfig, Tag, Task, TaskStatus, URGENT_TAG, task_ref,
};
use chrono::{NaiveDate, NaiveDateTime};
use rusqlite::types::{ToSqlOutput, Type, Value, ValueRef};
use rusqlite::{
    Connection, OptionalExtension, Params, Row, Transaction, TransactionBehavior, params,
    params_from_iter,
};
use std::collections::{HashMap, HashSet};
use std::path::Path;

mod groups;
mod migrate;
mod pushes;
mod reporting;
mod sessions;
mod sqltypes;
mod tasks;
#[cfg(test)]
mod tests;

pub(crate) use sqltypes::datetime;

/// Names bound per `ids_by_name` query (older SQLite allows 999 variables).
const NAME_CHUNK: usize = 500;

/// The condition of the unfinished tasks (a task is finished when `done`).
const NOT_DONE: &str = "status <> 'done'";

/// A `UNIQUE` violation (by extended result code, not message) on a unique-name table is
/// [`IterError::NameTaken`]; anything else is a database error.
fn write_error(error: rusqlite::Error, table: &str) -> IterError {
    let unique = matches!(
        &error,
        rusqlite::Error::SqliteFailure(e, _)
            if e.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
    );
    let kind = match table {
        "organizations" => "organization",
        "projects" => "project",
        "tasks" => "task",
        "boards" => "board",
        "tags" => "tag",
        _ => return error.into(),
    };
    if unique {
        IterError::NameTaken { kind }
    } else {
        error.into()
    }
}

const DATETIME_FMT: &str = "%Y-%m-%d %H:%M:%S";

fn dt_to_str(dt: NaiveDateTime) -> String {
    dt.format(DATETIME_FMT).to_string()
}

fn str_to_dt(s: &str) -> chrono::ParseResult<NaiveDateTime> {
    NaiveDateTime::parse_from_str(s, DATETIME_FMT)
}

/// `ids` as a JSON array for `json_each`: one bound variable however many ids.
fn json_ids(ids: &[i64]) -> String {
    format!(
        "[{}]",
        ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
    )
}

/// The half-open `[from 00:00:00, to+1day 00:00:00)` window of a date range, as stored text.
/// Half-open so no sub-second tail is dropped; an absent `from` is open-ended. Text comparison
/// is exact because [`DATETIME_FMT`] is fixed-width and zero-padded.
fn range_bounds(from: Option<NaiveDate>, to: NaiveDate) -> (String, String) {
    let lower = from.map_or_else(
        || "0000-01-01 00:00:00".to_string(),
        |from| format!("{from} 00:00:00"),
    );
    let upper = to.succ_opt().map_or_else(
        || "9999-12-31 23:59:59".to_string(),
        |after| format!("{after} 00:00:00"),
    );
    (lower, upper)
}

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

/// `SELECT id, {columns} FROM {table} {tail}`; the only place a select list is spelled out.
fn select_sql<T: Table>(tail: &str) -> String {
    format!(
        "SELECT id, {} FROM {} {tail}",
        T::COLUMNS.join(", "),
        T::NAME
    )
}

/// `INSERT INTO {table} ({columns}) VALUES (?1, ..., ?n)` in `COLUMNS` order.
fn insert_sql<T: Table>() -> String {
    let placeholders: Vec<String> = (1..=T::COLUMNS.len()).map(|i| format!("?{i}")).collect();
    format!(
        "INSERT INTO {} ({}) VALUES ({})",
        T::NAME,
        T::COLUMNS.join(", "),
        placeholders.join(", ")
    )
}

/// `UPDATE {table} SET c1 = ?1, ... WHERE id = ?n+1`: `insert_sql`'s bindings plus the id.
fn update_sql<T: Table>() -> String {
    let assignments: Vec<String> = T::COLUMNS
        .iter()
        .enumerate()
        .map(|(i, column)| format!("{column} = ?{}", i + 1))
        .collect();
    format!(
        "UPDATE {} SET {} WHERE id = ?{}",
        T::NAME,
        assignments.join(", "),
        T::COLUMNS.len() + 1
    )
}

fn delete_sql<T: Table>() -> String {
    format!("DELETE FROM {} WHERE id = ?1", T::NAME)
}

/// Opens the configured database or exits with the reason: nothing downstream works without it.
pub fn open_db() -> Db {
    let path = config().db_path().unwrap_or_else(|e| {
        eprintln!("error: {e}");
        std::process::exit(1);
    });
    Db::open(&path).unwrap_or_else(|e| {
        eprintln!("failed to open database at {}: {e}", path.display());
        std::process::exit(1);
    })
}

pub struct Db {
    conn: Connection,
}

/// Switches to WAL (stored in the file). On a brand-new file two processes can race and SQLite
/// reports the loser busy at once, bypassing `busy_timeout`, so retry for about that long.
fn enable_wal(conn: &Connection) -> Result<()> {
    const ATTEMPTS: u32 = 500;
    for attempt in 1.. {
        match conn.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(())) {
            Err(rusqlite::Error::SqliteFailure(e, _))
                if e.code == rusqlite::ErrorCode::DatabaseBusy && attempt < ATTEMPTS =>
            {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            result => return Ok(result?),
        }
    }
    unreachable!("the last attempt returns")
}

impl Db {
    /// Opens the file, creating it and its directory (which may not exist yet).
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA busy_timeout = 5000;
             PRAGMA foreign_keys = ON;",
        )?;
        enable_wal(&conn)?;
        let db = Db { conn };
        db.migrate()?;
        Ok(db)
    }

    fn find_one<T: Table>(&self, tail: &str, params: impl Params) -> Result<Option<T>> {
        Ok(self
            .conn
            .prepare_cached(&select_sql::<T>(tail))?
            .query_row(params, T::from_row)
            .optional()?)
    }

    fn find_all<T: Table>(&self, tail: &str, params: impl Params) -> Result<Vec<T>> {
        self.query_all(&select_sql::<T>(tail), params, T::from_row)
    }

    fn query_all<R>(
        &self,
        sql: &str,
        params: impl Params,
        map: impl FnMut(&Row) -> rusqlite::Result<R>,
    ) -> Result<Vec<R>> {
        let mut stmt = self.conn.prepare_cached(sql)?;
        let rows = stmt
            .query_map(params, map)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn find_by_name<T: Named>(&self, name: &str) -> Result<Option<T>> {
        self.find_one("WHERE name = ?1", params![name])
    }

    pub fn resolve<T: Named>(&self, name: &str) -> Result<T> {
        self.find_by_name(name)?.ok_or_else(|| IterError::NotFound {
            kind: T::KIND,
            name: name.to_string(),
        })
    }

    /// Ids of the `T`s named `names`, all or nothing: an unknown name errors before anything is
    /// written. Chunked by [`NAME_CHUNK`].
    pub fn ids_by_name<T: Named>(&self, names: &[String]) -> Result<Vec<i64>> {
        let mut seen = HashSet::new();
        let wanted: Vec<&str> = names
            .iter()
            .map(String::as_str)
            .filter(|n| seen.insert(*n))
            .collect();
        let mut found: HashMap<String, i64> = HashMap::with_capacity(wanted.len());
        for chunk in wanted.chunks(NAME_CHUNK) {
            let marks = vec!["?"; chunk.len()].join(",");
            found.extend(self.query_all(
                &format!("SELECT name, id FROM {} WHERE name IN ({marks})", T::NAME),
                params_from_iter(chunk),
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )?);
        }
        wanted
            .iter()
            .map(|n| {
                found.get(*n).copied().ok_or_else(|| IterError::NotFound {
                    kind: T::KIND,
                    name: n.to_string(),
                })
            })
            .collect()
    }

    /// Every `T`'s name in listing order. Reads only `name`: completion runs this on every TAB.
    pub fn names<T: Named>(&self) -> Result<Vec<String>> {
        self.query_all(
            &format!("SELECT name FROM {} {}", T::NAME, T::LIST_TAIL),
            [],
            |row| row.get(0),
        )
    }

    /// One transaction; an error rolls everything back. `writes` must not open its own.
    pub fn atomically<R>(&self, writes: impl FnOnce() -> Result<R>) -> Result<R> {
        let tx = self.conn.unchecked_transaction()?;
        let done = writes()?;
        tx.commit()?;
        Ok(done)
    }

    /// One transaction to batch commits (one fsync). Work done is committed even if `writes` then
    /// fails. `writes` must not open its own.
    pub fn batched<R>(&self, writes: impl FnOnce() -> Result<R>) -> Result<R> {
        let tx = self.conn.unchecked_transaction()?;
        let result = writes();
        tx.commit()?;
        result
    }

    pub fn insert<T: Table>(&self, item: &T) -> Result<i64> {
        self.conn
            .prepare_cached(&insert_sql::<T>())?
            .execute(params_from_iter(item.values()?))
            .map_err(|e| write_error(e, T::NAME))?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn update<T: Table>(&self, id: i64, item: &T) -> Result<()> {
        let mut values = item.values()?;
        values.push(id.into());
        self.conn
            .prepare_cached(&update_sql::<T>())?
            .execute(params_from_iter(values))
            .map_err(|e| write_error(e, T::NAME))?;
        Ok(())
    }

    pub fn delete<T: Table>(&self, id: i64) -> Result<()> {
        self.conn
            .prepare_cached(&delete_sql::<T>())?
            .execute(params![id])?;
        Ok(())
    }

    pub fn get<T: Table>(&self, id: i64) -> Result<Option<T>> {
        self.find_one("WHERE id = ?1", params![id])
    }

    #[cfg_attr(not(feature = "web"), allow(dead_code))]
    pub fn list<T: Table>(&self) -> Result<Vec<T>> {
        self.find_all(T::LIST_TAIL, [])
    }
}
