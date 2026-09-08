use crate::error::Result;
use crate::models::{Organization, Project, Session, SessionConfig, Task, TaskStatus};
use chrono::NaiveDateTime;
use rusqlite::types::{Value, ValueRef};
use rusqlite::{Connection, OptionalExtension, Params, Row, params, params_from_iter};
use std::path::Path;

const DATETIME_FMT: &str = "%Y-%m-%d %H:%M:%S";

fn dt_to_str(dt: NaiveDateTime) -> String {
    dt.format(DATETIME_FMT).to_string()
}

fn str_to_dt(s: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(s, DATETIME_FMT)
        .unwrap_or_else(|_| panic!("bad datetime in db: {s}"))
}

/// How one type is laid out in SQLite: which table it lives in, which
/// columns it writes, how a row is read back, and how an instance is bound
/// for writing. The blanket `impl<T: Table> Repository<T> for Db` below
/// derives every statement (`SELECT`/`INSERT`/`UPDATE`/`DELETE`) from these
/// four items, so no CRUD SQL is written by hand per table.
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

    /// Trailing clause for a bare `Repository::list`, e.g. `"ORDER BY name"`.
    const LIST_TAIL: &'static str = "";

    /// Reads a row shaped `id, {COLUMNS}` -- the shape `select_sql` builds.
    fn from_row(row: &Row) -> rusqlite::Result<Self>;

    /// This item's column values, in `COLUMNS` order.
    fn values(&self) -> Vec<Value>;
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

/// Generic storage surface, available for every [`Table`] via one blanket
/// implementation against the single `Db`/SQLite backend.
pub trait Repository<T> {
    fn insert(&self, item: &T) -> Result<i64>;
    fn update(&self, id: i64, item: &T) -> Result<()>;
    fn delete(&self, id: i64) -> Result<()>;
    fn get(&self, id: i64) -> Result<Option<T>>;
    fn list(&self) -> Result<Vec<T>>;
}

pub struct Db {
    conn: Connection,
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
            "PRAGMA journal_mode = WAL;
             PRAGMA busy_timeout = 5000;
             PRAGMA foreign_keys = ON;",
        )?;
        let db = Db { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        self.rename_legacy_tables()?;
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS organizations (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                name            TEXT NOT NULL UNIQUE,
                description     TEXT NOT NULL DEFAULT '',
                github          INTEGER NOT NULL DEFAULT 0,
                tmux            INTEGER NOT NULL DEFAULT 1,
                auto_branch     INTEGER NOT NULL DEFAULT 1,
                branch_template TEXT NOT NULL DEFAULT 'feat/{task}'
             );
             CREATE TABLE IF NOT EXISTS projects (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                organization_id INTEGER REFERENCES organizations(id) ON DELETE SET NULL,
                name            TEXT NOT NULL UNIQUE,
                description     TEXT NOT NULL DEFAULT '',
                base_path       TEXT NOT NULL,
                github          INTEGER NOT NULL DEFAULT 0,
                tmux            INTEGER NOT NULL DEFAULT 1,
                auto_branch     INTEGER NOT NULL DEFAULT 1,
                branch_template TEXT NOT NULL DEFAULT 'feat/{task}'
             );
             CREATE TABLE IF NOT EXISTS tasks (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                project_id    INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                name          TEXT NOT NULL,
                description   TEXT NOT NULL DEFAULT '',
                github_issue  INTEGER,
                status        TEXT NOT NULL DEFAULT 'queue' CHECK (status IN ('queue', 'wip', 'done')),
                branch_prefix TEXT NOT NULL DEFAULT '',
                UNIQUE (project_id, name)
             );
             CREATE TABLE IF NOT EXISTS session_configs (
                id                INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id           INTEGER NOT NULL UNIQUE REFERENCES tasks(id) ON DELETE CASCADE,
                tmux_session_name TEXT,
                github_branch     TEXT,
                worktree_path     TEXT
             );
             CREATE TABLE IF NOT EXISTS sessions (
                id      INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                start   TEXT NOT NULL,
                end     TEXT,
                message TEXT
             );",
        )?;
        self.add_missing_columns()?;
        Ok(())
    }

    /// Columns added to a table after it was first created. `CREATE TABLE
    /// IF NOT EXISTS` above is a no-op on a database that already has the
    /// table, so a field added later needs its own `ALTER TABLE` for the
    /// databases already in the wild; each one is guarded on the column not
    /// being there yet, making this a no-op on a fresh database too.
    ///
    /// `organization_id` is `ON DELETE SET NULL`, not `CASCADE`: an
    /// organization is a grouping, so deleting one must not take its
    /// projects -- and every task and session under them -- down with it.
    /// They simply stop belonging to one, which is a state every project is
    /// already allowed to be in.
    fn add_missing_columns(&self) -> Result<()> {
        const ADDED: [(&str, &str, &str); 2] = [
            (
                "projects",
                "organization_id",
                "INTEGER REFERENCES organizations(id) ON DELETE SET NULL",
            ),
            ("tasks", "branch_prefix", "TEXT NOT NULL DEFAULT ''"),
        ];
        for (table, column, decl) in ADDED {
            if !self.column_exists(table, column)? {
                self.conn
                    .execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {column} {decl};"))?;
            }
        }
        Ok(())
    }

    fn column_exists(&self, table: &str, column: &str) -> Result<bool> {
        let mut stmt = self.conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let names = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(names.iter().any(|name| name == column))
    }

    /// One-time rename from the pre-rename schema (`Session`/`Record`
    /// structs, backed by tables `sessions`/`records`) to the current one
    /// (`SessionConfig`/`Session`, backed by tables `session_configs`/
    /// `sessions`). Each rename is guarded on its target name not already
    /// existing, so this is a no-op once applied, and a no-op on a fresh
    /// database that never had the old tables to begin with. Order matters:
    /// the old `sessions` table is renamed out of the way first, freeing
    /// that name for the old `records` table to take.
    fn rename_legacy_tables(&self) -> Result<()> {
        if self.table_exists("sessions")? && !self.table_exists("session_configs")? {
            self.conn
                .execute_batch("ALTER TABLE sessions RENAME TO session_configs;")?;
        }
        if self.table_exists("records")? && !self.table_exists("sessions")? {
            self.conn
                .execute_batch("ALTER TABLE records RENAME TO sessions;")?;
        }
        Ok(())
    }

    fn table_exists(&self, name: &str) -> Result<bool> {
        Ok(self
            .conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
                params![name],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    }

    // ---- generic query helpers -------------------------------------------

    /// The single row matching `tail` (a `WHERE ...` clause), if any.
    fn find_one<T: Table>(&self, tail: &str, params: impl Params) -> Result<Option<T>> {
        Ok(self
            .conn
            .query_row(&select_sql::<T>(tail), params, T::from_row)
            .optional()?)
    }

    /// Every row matching `tail` (a `WHERE`/`ORDER BY` clause).
    fn find_all<T: Table>(&self, tail: &str, params: impl Params) -> Result<Vec<T>> {
        let mut stmt = self.conn.prepare(&select_sql::<T>(tail))?;
        let rows = stmt
            .query_map(params, T::from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    // ---- finders (lookups the generic CRUD surface doesn't cover) ---------

    pub fn find_project_by_name(&self, name: &str) -> Result<Option<Project>> {
        self.find_one("WHERE name = ?1", params![name])
    }

    pub fn find_organization_by_name(&self, name: &str) -> Result<Option<Organization>> {
        self.find_one("WHERE name = ?1", params![name])
    }

    /// Every project belonging to `organization_id`, in name order -- the
    /// roster an organization report walks.
    pub fn projects_for_organization(&self, organization_id: i64) -> Result<Vec<Project>> {
        self.find_all(
            "WHERE organization_id = ?1 ORDER BY name",
            params![organization_id],
        )
    }

    pub fn find_task(&self, project_id: i64, task_name: &str) -> Result<Option<Task>> {
        self.find_one(
            "WHERE project_id = ?1 AND name = ?2",
            params![project_id, task_name],
        )
    }

    pub fn tasks_for_project(&self, project_id: i64) -> Result<Vec<Task>> {
        self.find_all("WHERE project_id = ?1 ORDER BY name", params![project_id])
    }

    pub fn find_session_config_by_task(&self, task_id: i64) -> Result<Option<SessionConfig>> {
        self.find_one("WHERE task_id = ?1", params![task_id])
    }

    pub fn find_session_config_by_tmux_name(
        &self,
        tmux_name: &str,
    ) -> Result<Option<SessionConfig>> {
        self.find_one("WHERE tmux_session_name = ?1", params![tmux_name])
    }

    pub fn sessions_for_task(&self, task_id: i64) -> Result<Vec<Session>> {
        self.find_all("WHERE task_id = ?1", params![task_id])
    }

    pub fn open_session_for_task(&self, task_id: i64) -> Result<Option<Session>> {
        self.find_one("WHERE task_id = ?1 AND end IS NULL", params![task_id])
    }
}

impl<T: Table> Repository<T> for Db {
    fn insert(&self, item: &T) -> Result<i64> {
        self.conn
            .execute(&insert_sql::<T>(), params_from_iter(item.values()))?;
        Ok(self.conn.last_insert_rowid())
    }

    fn update(&self, id: i64, item: &T) -> Result<()> {
        let mut values = item.values();
        values.push(id.into());
        self.conn
            .execute(&update_sql::<T>(), params_from_iter(values))?;
        Ok(())
    }

    fn delete(&self, id: i64) -> Result<()> {
        self.conn.execute(&delete_sql::<T>(), params![id])?;
        Ok(())
    }

    fn get(&self, id: i64) -> Result<Option<T>> {
        self.find_one("WHERE id = ?1", params![id])
    }

    fn list(&self) -> Result<Vec<T>> {
        self.find_all(T::LIST_TAIL, [])
    }
}

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
    fn from_sql(row: &Row, index: usize) -> rusqlite::Result<Self> {
        Ok(str_to_dt(&row.get::<_, String>(index)?))
    }

    fn to_sql(&self) -> Value {
        Value::Text(dt_to_str(*self))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ProjectDefaults;

    fn dt(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M")
            .unwrap_or_else(|e| panic!("bad test fixture datetime '{s}': {e}"))
    }

    /// A fresh, empty, migrated database held entirely in memory.
    fn db() -> Db {
        Db::open(":memory:").expect("in-memory database opens")
    }

    fn project(name: &str) -> Project {
        Project {
            id: None,
            organization_id: None,
            name: name.to_string(),
            description: "notes".to_string(),
            base_path: format!("/tmp/{name}"),
            github: true,
            tmux: false,
            auto_branch: true,
            branch_template: "fix/{task}".to_string(),
        }
    }

    fn insert_project(db: &Db, name: &str) -> i64 {
        Repository::<Project>::insert(db, &project(name)).expect("project inserts")
    }

    fn insert_task(db: &Db, project_id: i64, name: &str) -> i64 {
        let task = Task {
            id: None,
            project_id,
            name: name.to_string(),
            description: "task notes".to_string(),
            github_issue: Some(7),
            status: TaskStatus::Wip,
            branch_prefix: "fix/".to_string(),
        };
        Repository::<Task>::insert(db, &task).expect("task inserts")
    }

    #[test]
    fn generated_statements_match_the_table_declaration() {
        assert_eq!(
            select_sql::<Session>("WHERE task_id = ?1"),
            "SELECT id, task_id, start, end, message FROM sessions WHERE task_id = ?1"
        );
        assert_eq!(
            insert_sql::<Session>(),
            "INSERT INTO sessions (task_id, start, end, message) VALUES (?1, ?2, ?3, ?4)"
        );
        assert_eq!(
            update_sql::<Session>(),
            "UPDATE sessions SET task_id = ?1, start = ?2, end = ?3, message = ?4 WHERE id = ?5"
        );
        assert_eq!(
            delete_sql::<Session>(),
            "DELETE FROM sessions WHERE id = ?1"
        );
    }

    #[test]
    fn organization_statements_carry_the_downstream_defaults() {
        assert_eq!(
            insert_sql::<Organization>(),
            "INSERT INTO organizations (name, description, github, tmux, auto_branch, \
             branch_template) VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
        );
        assert_eq!(
            select_sql::<Organization>("WHERE name = ?1"),
            "SELECT id, name, description, github, tmux, auto_branch, branch_template \
             FROM organizations WHERE name = ?1"
        );
    }

    /// Every column a struct declares must actually exist in the migrated
    /// schema. This is the guard that `#[derive(Table)]` needs: because the
    /// derive takes the columns straight off the struct's fields, adding a
    /// field silently adds a column, and forgetting the matching change to
    /// `migrate` would otherwise surface as a runtime "no such column".
    #[test]
    fn every_declared_column_exists_in_the_migrated_schema() {
        let db = db();

        fn columns_of(db: &Db, table: &str) -> Vec<String> {
            let mut stmt = db
                .conn
                .prepare(&format!("PRAGMA table_info({table})"))
                .expect("table_info prepares");
            let names = stmt
                .query_map([], |row| row.get::<_, String>(1))
                .expect("table_info runs")
                .collect::<rusqlite::Result<Vec<_>>>()
                .expect("table_info rows read");
            assert!(!names.is_empty(), "table `{table}` does not exist");
            names
        }

        fn check<T: Table>(db: &Db) {
            let actual = columns_of(db, T::NAME);
            assert!(
                actual.iter().any(|c| c == "id"),
                "table `{}` has no `id` column, but every row is read as `id` first",
                T::NAME
            );
            for column in T::COLUMNS {
                assert!(
                    actual.iter().any(|c| c == column),
                    "`{}` declares column `{column}`, which the schema doesn't have (got {actual:?})",
                    T::NAME
                );
            }
        }

        check::<Organization>(&db);
        check::<Project>(&db);
        check::<Task>(&db);
        check::<SessionConfig>(&db);
        check::<Session>(&db);
    }

    /// Every table binds exactly as many values as it declares columns.
    /// `#[derive(Table)]` makes this true by construction; the test stays
    /// as the guard for any `Table` impl written by hand, where `COLUMNS`
    /// and `values` would again be two lists that could disagree.
    #[test]
    fn every_table_binds_one_value_per_column() {
        fn check<T: Table>(item: &T) {
            assert_eq!(
                item.values().len(),
                T::COLUMNS.len(),
                "{} binds a different number of values than it declares columns",
                T::NAME
            );
        }
        check(&project("alpha"));
        check(&Organization::template(&ProjectDefaults::default()));
        check(&Task {
            id: None,
            project_id: 1,
            name: "t".to_string(),
            description: String::new(),
            github_issue: None,
            status: TaskStatus::Queue,
            branch_prefix: String::new(),
        });
        check(&SessionConfig {
            id: None,
            task_id: 1,
            tmux_session_name: None,
            github_branch: None,
            worktree_path: None,
        });
        check(&Session {
            id: None,
            task_id: 1,
            start: dt("2026-09-01 09:00"),
            end: None,
            message: None,
        });
    }

    #[test]
    fn project_round_trips_every_field() {
        let db = db();
        let id = insert_project(&db, "alpha");
        let loaded = Repository::<Project>::get(&db, id)
            .expect("get succeeds")
            .expect("the row just inserted exists");

        assert_eq!(loaded.id, Some(id));
        assert_eq!(loaded.name, "alpha");
        assert_eq!(loaded.description, "notes");
        assert_eq!(loaded.base_path, "/tmp/alpha");
        // The bools matter most: they cross a bool -> INTEGER -> bool round
        // trip, and `tmux` is deliberately the non-default `false` here.
        assert!(loaded.github);
        assert!(!loaded.tmux);
        assert!(loaded.auto_branch);
        assert_eq!(loaded.branch_template, "fix/{task}");
    }

    /// Databases in the wild carry columns this build no longer knows
    /// about -- `projects.auto_issue`, left behind by a removed feature,
    /// is one. The generated statements name their columns explicitly, so
    /// an extra column with a default is simply not written; this pins
    /// that, since a `SELECT *`/positional binding would break on it.
    #[test]
    fn an_unknown_extra_column_does_not_break_writes() {
        let db = db();
        db.conn
            .execute_batch(
                "DROP TABLE projects;
                 CREATE TABLE projects (
                    id              INTEGER PRIMARY KEY AUTOINCREMENT,
                    organization_id INTEGER REFERENCES organizations(id) ON DELETE SET NULL,
                    name            TEXT NOT NULL UNIQUE,
                    description     TEXT NOT NULL DEFAULT '',
                    base_path       TEXT NOT NULL,
                    github          INTEGER NOT NULL DEFAULT 0,
                    tmux            INTEGER NOT NULL DEFAULT 1,
                    auto_branch     INTEGER NOT NULL DEFAULT 1,
                    branch_template TEXT NOT NULL DEFAULT 'feat/{task}',
                    auto_issue      INTEGER NOT NULL DEFAULT 0
                 );",
            )
            .expect("drifted schema is created");

        let id = insert_project(&db, "alpha");
        Repository::<Project>::update(&db, id, &project("renamed")).expect("update succeeds");

        let loaded = Repository::<Project>::get(&db, id)
            .expect("get succeeds")
            .expect("the row exists");
        assert_eq!(loaded.name, "renamed");
    }

    /// The mirror of `an_unknown_extra_column_does_not_break_writes`: a
    /// database created before `branch_prefix` existed is *missing* a
    /// column the struct declares, which no amount of explicit naming
    /// survives -- `migrate` has to add it.
    #[test]
    fn a_pre_existing_tasks_table_gains_the_branch_prefix_column() {
        let db = db();
        db.conn
            .execute_batch(
                "DROP TABLE tasks;
                 CREATE TABLE tasks (
                    id           INTEGER PRIMARY KEY AUTOINCREMENT,
                    project_id   INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                    name         TEXT NOT NULL,
                    description  TEXT NOT NULL DEFAULT '',
                    github_issue INTEGER,
                    status       TEXT NOT NULL DEFAULT 'queue',
                    UNIQUE (project_id, name)
                 );",
            )
            .expect("the old schema is created");
        db.migrate().expect("migrating adds the column");

        let project_id = insert_project(&db, "alpha");
        let id = insert_task(&db, project_id, "build it");
        let loaded = Repository::<Task>::get(&db, id)
            .expect("get succeeds")
            .expect("the row just inserted exists");
        assert_eq!(loaded.branch_prefix, "fix/");
    }

    #[test]
    fn project_update_rewrites_every_column() {
        let db = db();
        let id = insert_project(&db, "alpha");

        let mut changed = project("renamed");
        changed.github = false;
        changed.tmux = true;
        changed.auto_branch = false;
        changed.branch_template = "chore/{task}".to_string();
        Repository::<Project>::update(&db, id, &changed).expect("update succeeds");

        let loaded = Repository::<Project>::get(&db, id)
            .expect("get succeeds")
            .expect("the updated row exists");
        assert_eq!(loaded.name, "renamed");
        assert!(!loaded.github);
        assert!(loaded.tmux);
        assert!(!loaded.auto_branch);
        assert_eq!(loaded.branch_template, "chore/{task}");
    }

    #[test]
    fn project_list_is_ordered_by_name() {
        let db = db();
        insert_project(&db, "charlie");
        insert_project(&db, "alpha");
        insert_project(&db, "bravo");

        let names: Vec<String> = Repository::<Project>::list(&db)
            .expect("list succeeds")
            .into_iter()
            .map(|p| p.name)
            .collect();
        assert_eq!(names, ["alpha", "bravo", "charlie"]);
    }

    #[test]
    fn find_project_by_name_matches_and_misses() {
        let db = db();
        insert_project(&db, "alpha");
        assert!(
            db.find_project_by_name("alpha")
                .expect("lookup succeeds")
                .is_some()
        );
        assert!(
            db.find_project_by_name("nope")
                .expect("lookup succeeds")
                .is_none()
        );
    }

    #[test]
    fn task_round_trips_and_is_found_by_project_and_name() {
        let db = db();
        let project_id = insert_project(&db, "alpha");
        let task_id = insert_task(&db, project_id, "build it");

        let found = db
            .find_task(project_id, "build it")
            .expect("lookup succeeds")
            .expect("the task just inserted exists");
        assert_eq!(found.id, Some(task_id));
        assert_eq!(found.project_id, project_id);
        assert_eq!(found.description, "task notes");
        assert_eq!(found.github_issue, Some(7));
        assert_eq!(found.status, TaskStatus::Wip);
        assert_eq!(found.branch_prefix, "fix/");
    }

    #[test]
    fn task_github_issue_round_trips_as_null_when_absent() {
        let db = db();
        let project_id = insert_project(&db, "alpha");
        let task = Task {
            id: None,
            project_id,
            name: "no issue".to_string(),
            description: String::new(),
            github_issue: None,
            status: TaskStatus::Queue,
            branch_prefix: String::new(),
        };
        let id = Repository::<Task>::insert(&db, &task).expect("task inserts");
        let loaded = Repository::<Task>::get(&db, id)
            .expect("get succeeds")
            .expect("the row just inserted exists");
        assert_eq!(loaded.github_issue, None);
        assert_eq!(loaded.status, TaskStatus::Queue);
    }

    #[test]
    fn deleting_a_project_cascades_to_its_tasks() {
        let db = db();
        let project_id = insert_project(&db, "alpha");
        insert_task(&db, project_id, "build it");

        Repository::<Project>::delete(&db, project_id).expect("delete succeeds");
        assert!(
            db.tasks_for_project(project_id)
                .expect("lookup succeeds")
                .is_empty()
        );
    }

    #[test]
    fn session_config_round_trips_its_optional_columns() {
        let db = db();
        let project_id = insert_project(&db, "alpha");
        let task_id = insert_task(&db, project_id, "build it");

        let config = SessionConfig {
            id: None,
            task_id,
            tmux_session_name: Some("alpha/build-it".to_string()),
            github_branch: None,
            worktree_path: Some("/tmp/wt".to_string()),
        };
        Repository::<SessionConfig>::insert(&db, &config).expect("config inserts");

        let by_task = db
            .find_session_config_by_task(task_id)
            .expect("lookup succeeds")
            .expect("the config just inserted exists");
        assert_eq!(by_task.tmux_session_name.as_deref(), Some("alpha/build-it"));
        assert_eq!(by_task.github_branch, None);
        assert_eq!(by_task.worktree_path.as_deref(), Some("/tmp/wt"));

        let by_name = db
            .find_session_config_by_tmux_name("alpha/build-it")
            .expect("lookup succeeds")
            .expect("the config is findable by its tmux name");
        assert_eq!(by_name.id, by_task.id);
    }

    #[test]
    fn open_session_is_the_one_without_an_end() {
        let db = db();
        let project_id = insert_project(&db, "alpha");
        let task_id = insert_task(&db, project_id, "build it");

        let closed = Session {
            id: None,
            task_id,
            start: dt("2026-09-01 09:00"),
            end: Some(dt("2026-09-01 10:00")),
            message: Some("did a thing".to_string()),
        };
        let open = Session {
            id: None,
            task_id,
            start: dt("2026-09-01 11:00"),
            end: None,
            message: None,
        };
        Repository::<Session>::insert(&db, &closed).expect("closed session inserts");
        let open_id = Repository::<Session>::insert(&db, &open).expect("open session inserts");

        let found = db
            .open_session_for_task(task_id)
            .expect("lookup succeeds")
            .expect("there is an open session");
        assert_eq!(found.id, Some(open_id));
        assert!(found.is_ongoing());
        assert_eq!(found.start, dt("2026-09-01 11:00"));
    }

    #[test]
    fn closing_a_session_persists_its_end_and_message() {
        let db = db();
        let project_id = insert_project(&db, "alpha");
        let task_id = insert_task(&db, project_id, "build it");

        let session = Session {
            id: None,
            task_id,
            start: dt("2026-09-01 09:00"),
            end: None,
            message: None,
        };
        let id = Repository::<Session>::insert(&db, &session).expect("session inserts");

        let mut closed = session;
        closed.end = Some(dt("2026-09-01 10:30"));
        closed.message = Some("wrapped up".to_string());
        Repository::<Session>::update(&db, id, &closed).expect("update succeeds");

        let loaded = Repository::<Session>::get(&db, id)
            .expect("get succeeds")
            .expect("the updated row exists");
        assert_eq!(loaded.end, Some(dt("2026-09-01 10:30")));
        assert_eq!(loaded.message.as_deref(), Some("wrapped up"));
        assert!(
            db.open_session_for_task(task_id)
                .expect("lookup succeeds")
                .is_none()
        );
    }

    /// The union a project-level report sums is built by walking
    /// `tasks_for_project` and asking `sessions_for_task` for each -- so
    /// what has to hold is that neither leaks across a project boundary.
    #[test]
    fn a_projects_sessions_are_its_own_tasks_and_no_others() {
        let db = db();
        let project_id = insert_project(&db, "alpha");
        let one = insert_task(&db, project_id, "one");
        let two = insert_task(&db, project_id, "two");

        // A second project's session must not leak into the union.
        let other_project = insert_project(&db, "beta");
        let other_task = insert_task(&db, other_project, "elsewhere");

        for task_id in [one, two, other_task] {
            let session = Session {
                id: None,
                task_id,
                start: dt("2026-09-01 09:00"),
                end: Some(dt("2026-09-01 10:00")),
                message: None,
            };
            Repository::<Session>::insert(&db, &session).expect("session inserts");
        }

        let mut task_ids: Vec<i64> = db
            .tasks_for_project(project_id)
            .expect("lookup succeeds")
            .into_iter()
            .flat_map(|t| {
                db.sessions_for_task(t.id.expect("an inserted task has an id"))
                    .expect("lookup succeeds")
            })
            .map(|s| s.task_id)
            .collect();
        task_ids.sort_unstable();
        assert_eq!(task_ids, [one, two]);

        assert_eq!(db.sessions_for_task(one).expect("lookup succeeds").len(), 1);
    }

    fn insert_organization(db: &Db, name: &str) -> i64 {
        let mut organization = Organization::template(&ProjectDefaults::default());
        organization.name = name.to_string();
        organization.github = true;
        organization.tmux = false;
        organization.branch_template = "chore/{task}".to_string();
        Repository::<Organization>::insert(db, &organization).expect("organization inserts")
    }

    #[test]
    fn organization_round_trips_its_downstream_defaults() {
        let db = db();
        let id = insert_organization(&db, "acme");
        let loaded = Repository::<Organization>::get(&db, id)
            .expect("get succeeds")
            .expect("the row just inserted exists");
        assert_eq!(loaded.name, "acme");
        assert!(loaded.github);
        assert!(!loaded.tmux);
        assert!(loaded.auto_branch);
        assert_eq!(loaded.branch_template, "chore/{task}");
    }

    #[test]
    fn a_project_may_belong_to_an_organization_or_to_none() {
        let db = db();
        let organization = insert_organization(&db, "acme");

        let mut member = project("member");
        member.organization_id = Some(organization);
        let member_id = Repository::<Project>::insert(&db, &member).expect("project inserts");
        let loner_id = insert_project(&db, "loner");

        assert_eq!(
            Repository::<Project>::get(&db, member_id)
                .expect("get succeeds")
                .expect("row exists")
                .organization_id,
            Some(organization)
        );
        assert_eq!(
            Repository::<Project>::get(&db, loner_id)
                .expect("get succeeds")
                .expect("row exists")
                .organization_id,
            None,
            "a project without an organization must round-trip as NULL"
        );

        let roster: Vec<String> = db
            .projects_for_organization(organization)
            .expect("roster loads")
            .into_iter()
            .map(|p| p.name)
            .collect();
        assert_eq!(roster, ["member"], "only members are in the roster");
    }

    /// The guarantee that makes organizations safe to delete: the projects
    /// (and so every task and session under them) survive, merely stopping
    /// being members.
    #[test]
    fn deleting_an_organization_keeps_its_projects() {
        let db = db();
        let organization = insert_organization(&db, "acme");
        let mut member = project("member");
        member.organization_id = Some(organization);
        let project_id = Repository::<Project>::insert(&db, &member).expect("project inserts");
        let task_id = insert_task(&db, project_id, "build it");

        Repository::<Organization>::delete(&db, organization).expect("delete succeeds");

        let loaded = Repository::<Project>::get(&db, project_id)
            .expect("get succeeds")
            .expect("the project outlives its organization");
        assert_eq!(loaded.organization_id, None);
        assert_eq!(
            db.tasks_for_project(project_id)
                .expect("tasks load")
                .first()
                .map(|t| t.id),
            Some(Some(task_id)),
            "the project's tasks must survive too"
        );
    }

    /// A database written before organizations existed has a `projects`
    /// table with no `organization_id`, and `CREATE TABLE IF NOT EXISTS`
    /// won't add it -- so `migrate` has to.
    #[test]
    fn an_older_database_gains_the_organization_column() {
        let db = db();
        db.conn
            .execute_batch(
                "DROP TABLE projects;
                 CREATE TABLE projects (
                    id              INTEGER PRIMARY KEY AUTOINCREMENT,
                    name            TEXT NOT NULL UNIQUE,
                    description     TEXT NOT NULL DEFAULT '',
                    base_path       TEXT NOT NULL,
                    github          INTEGER NOT NULL DEFAULT 0,
                    tmux            INTEGER NOT NULL DEFAULT 1,
                    auto_branch     INTEGER NOT NULL DEFAULT 1,
                    branch_template TEXT NOT NULL DEFAULT 'feat/{task}'
                 );",
            )
            .expect("pre-organization schema is created");
        assert!(!db.column_exists("projects", "organization_id").unwrap());

        db.migrate().expect("migration succeeds");

        assert!(db.column_exists("projects", "organization_id").unwrap());
        // ...and the column is usable, defaulting to "no organization".
        let id = insert_project(&db, "legacy");
        assert_eq!(
            Repository::<Project>::get(&db, id)
                .expect("get succeeds")
                .expect("row exists")
                .organization_id,
            None
        );
    }

    #[test]
    fn a_fresh_database_migrates_without_legacy_tables() {
        let db = db();
        assert!(db.table_exists("projects").expect("check succeeds"));
        assert!(db.table_exists("tasks").expect("check succeeds"));
        assert!(db.table_exists("session_configs").expect("check succeeds"));
        assert!(db.table_exists("sessions").expect("check succeeds"));
        assert!(!db.table_exists("records").expect("check succeeds"));
    }

    #[test]
    fn legacy_tables_are_renamed_in_the_right_order() {
        // The pre-rename schema: `sessions` held what is now
        // `session_configs`, and `records` held what are now `sessions`.
        let db = db();
        db.conn
            .execute_batch(
                "DROP TABLE sessions;
                 DROP TABLE session_configs;
                 CREATE TABLE sessions (id INTEGER PRIMARY KEY, task_id INTEGER);
                 CREATE TABLE records (id INTEGER PRIMARY KEY, task_id INTEGER);",
            )
            .expect("legacy schema is created");

        db.migrate().expect("migration succeeds");

        assert!(db.table_exists("session_configs").expect("check succeeds"));
        assert!(db.table_exists("sessions").expect("check succeeds"));
        assert!(!db.table_exists("records").expect("check succeeds"));
    }
}
