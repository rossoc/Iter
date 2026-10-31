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

/// How many names one `ids_by_name` query binds (older SQLite builds allow
/// 999 bound variables).
const NAME_CHUNK: usize = 500;

/// The condition of the unfinished tasks (a task is finished when `done`).
const NOT_DONE: &str = "status <> 'done'";

/// The write error `error` to table `table` is: a `UNIQUE` violation (by
/// SQLite's extended result code, not its message) on one of the tables with
/// a unique name is [`IterError::NameTaken`]; anything else is a database
/// error.
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

/// `ids` as a JSON array, for a `json_each` subquery: one bound variable
/// however many ids there are.
fn json_ids(ids: &[i64]) -> String {
    format!(
        "[{}]",
        ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
    )
}

/// The half-open `[from 00:00:00, to+1day 00:00:00)` window a date range
/// covers, as the text `start` is stored in.
///
/// Half-open on purpose: an inclusive `<= to 23:59:59` would drop a session
/// started later in that final second if the stored format ever gained
/// sub-second precision. An absent `from` reaches back to the beginning of
/// time, which is what an open-ended range means.
///
/// Comparing these as text is exact rather than lucky: [`DATETIME_FMT`] is
/// fixed-width and zero-padded, so lexicographic order *is* chronological
/// order.
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

/// How one type is laid out in SQLite: which table it lives in, which
/// columns it writes, how a row is read back, and how an instance is bound
/// for writing. `Db`'s generic CRUD methods below derive every statement
/// (`SELECT`/`INSERT`/`UPDATE`/`DELETE`) from these items, so no CRUD SQL
/// is written by hand per table.
///
/// Don't implement this by hand -- put `#[derive(Table)]` on the struct.
/// The derive reads the columns off the struct's own fields, so `COLUMNS`,
/// the column indices `from_row` reads and the order `values` binds all
/// come from one place and cannot drift; there is no column list to
/// maintain alongside the struct at all.
pub(crate) trait Table: Sized {
    /// The table this type is stored in.
    const NAME: &'static str;

    /// The writable columns, in the order `values` binds them. `id` is
    /// deliberately absent: SQLite assigns it on insert, and it's the key
    /// (not a payload) on update.
    const COLUMNS: &'static [&'static str];

    /// Trailing clause for a bare `Db::list`, e.g. `"ORDER BY name"`.
    const LIST_TAIL: &'static str = "";

    /// Reads a row shaped `id, {COLUMNS}` -- the shape `select_sql` builds.
    fn from_row(row: &Row) -> rusqlite::Result<Self>;

    /// This item's column values, in `COLUMNS` order.
    fn values(&self) -> rusqlite::Result<Vec<ToSqlOutput<'_>>>;

    /// The primary key as stored on the item: `None` for one that hasn't
    /// been inserted yet (the blank template the editor opens), `Some`
    /// from the moment it comes back out of the database.
    fn row_id(&self) -> Option<i64>;

    /// The primary key of a row that came *out* of the database, where the
    /// `Option` above is always `Some`. Panicking here rather than at each
    /// call site is the point: the invariant is stated once, in the trait
    /// that owns identity, instead of at every use of an id.
    fn id(&self) -> i64 {
        self.row_id()
            .expect("a row loaded from the database always has an id")
    }
}

// ---- statement builders -------------------------------------------------
//
// The four statements every table needs, derived from its `Table` items.
// Kept together (and free functions rather than inline `format!`s) so the
// generated SQL can be asserted directly in the tests below.

/// `SELECT id, {columns} FROM {table} {tail}`, where `tail` is a caller's
/// `WHERE`/`ORDER BY` clause. The only place a select list is spelled out.
fn select_sql<T: Table>(tail: &str) -> String {
    format!(
        "SELECT id, {} FROM {} {tail}",
        T::COLUMNS.join(", "),
        T::NAME
    )
}

/// `INSERT INTO {table} ({columns}) VALUES (?1, ..., ?n)` -- `n` bindings
/// in `COLUMNS` order, which is the order `Table::values` returns them.
fn insert_sql<T: Table>() -> String {
    let placeholders: Vec<String> = (1..=T::COLUMNS.len()).map(|i| format!("?{i}")).collect();
    format!(
        "INSERT INTO {} ({}) VALUES ({})",
        T::NAME,
        T::COLUMNS.join(", "),
        placeholders.join(", ")
    )
}

/// `UPDATE {table} SET c1 = ?1, ... WHERE id = ?n+1` -- the same bindings
/// as `insert_sql`, with the row's id appended as the final one.
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

/// Opens the configured database, or exits reporting why. Every command
/// and every completion callback starts here: nothing downstream can do
/// anything useful without the database, so a failure to reach it is fatal
/// rather than threaded through as an error.
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

/// Puts the database in WAL mode. The mode is stored in the file, so this
/// is a no-op after the first run -- but on that first run, two processes
/// opening a brand-new file at once (a tmux hook and a web request, say)
/// can race to switch it, and SQLite reports the loser as busy straight
/// away rather than through `busy_timeout`. So a busy switch is retried,
/// for about as long as the timeout would have waited.
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
    /// Opens (creating if need be) the SQLite file at `path`, along with
    /// the directory holding it -- the configured location is allowed to
    /// be somewhere that doesn't exist yet, including the default
    /// `~/.config/iter` on a machine that has never run `iter`.
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

    /// The single row matching `tail` (a `WHERE ...` clause), if any.
    fn find_one<T: Table>(&self, tail: &str, params: impl Params) -> Result<Option<T>> {
        Ok(self
            .conn
            .prepare_cached(&select_sql::<T>(tail))?
            .query_row(params, T::from_row)
            .optional()?)
    }

    /// Every row matching `tail` (a `WHERE`/`ORDER BY` clause).
    fn find_all<T: Table>(&self, tail: &str, params: impl Params) -> Result<Vec<T>> {
        self.query_all(&select_sql::<T>(tail), params, T::from_row)
    }

    /// Every row `sql` returns, each read by `map` -- the one
    /// prepare/query/collect sequence every multi-row read goes through.
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

    /// The `T` named `name`, if any. Every table with a unique, user-facing
    /// `name` column is looked up exactly this way, so the lookup is
    /// generic rather than written once per entity.
    pub fn find_by_name<T: Named>(&self, name: &str) -> Result<Option<T>> {
        self.find_one("WHERE name = ?1", params![name])
    }

    /// The `T` named `name`, or the "no such <kind>" error.
    pub fn resolve<T: Named>(&self, name: &str) -> Result<T> {
        self.find_by_name(name)?.ok_or_else(|| IterError::NotFound {
            kind: T::KIND,
            name: name.to_string(),
        })
    }

    /// The ids of the `T`s named `names` (each once, in the order first
    /// listed), all or nothing: an unknown name is an error before anything
    /// is written, so a typo can't half-apply a roster. One query per
    /// [`NAME_CHUNK`] names (`name` is `UNIQUE`, so it is indexed), which
    /// keeps a long list under SQLite's limit on bound variables.
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

    /// Every `T`'s name, in `T`'s listing order -- what `iter <entity>
    /// list` prints and what shell completion offers. A listing of names is
    /// a query, so it lives here rather than in either caller. Only the
    /// `name` column is read: completion runs this on every TAB.
    pub fn names<T: Named>(&self) -> Result<Vec<String>> {
        self.query_all(
            &format!("SELECT name FROM {} {}", T::NAME, T::LIST_TAIL),
            [],
            |row| row.get(0),
        )
    }

    /// Runs `writes` as one transaction that commits only if it succeeds:
    /// an error rolls everything it wrote back. `writes` must not open a
    /// transaction of its own.
    pub fn atomically<R>(&self, writes: impl FnOnce() -> Result<R>) -> Result<R> {
        let tx = self.conn.unchecked_transaction()?;
        let done = writes()?;
        tx.commit()?;
        Ok(done)
    }

    /// Runs `writes` as one transaction, so a burst of row writes costs one
    /// commit (and one fsync) rather than one each. Whatever `writes` got
    /// done is committed even when it then fails -- this batches commits,
    /// it doesn't make the batch all-or-nothing. `writes` must not open a
    /// transaction of its own.
    pub fn batched<R>(&self, writes: impl FnOnce() -> Result<R>) -> Result<R> {
        let tx = self.conn.unchecked_transaction()?;
        let result = writes();
        tx.commit()?;
        result
    }

    /// Writes `item` as a new row and returns the id SQLite assigned it.
    pub fn insert<T: Table>(&self, item: &T) -> Result<i64> {
        self.conn
            .prepare_cached(&insert_sql::<T>())?
            .execute(params_from_iter(item.values()?))
            .map_err(|e| write_error(e, T::NAME))?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Overwrites row `id` with `item`'s columns.
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

    /// Every row of `T`, in `T::LIST_TAIL` order.
    #[cfg_attr(not(feature = "web"), allow(dead_code))]
    pub fn list<T: Table>(&self) -> Result<Vec<T>> {
        self.find_all(T::LIST_TAIL, [])
    }
}
