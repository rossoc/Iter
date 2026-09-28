use crate::config::config;
use crate::error::{IterError, Result};
use crate::models::{
    Board, Card, Duration, IMPORTANT_TAG, Named, Priority, Project, ProjectGroup, Session,
    SessionConfig, Tag, Task, TaskStatus, URGENT_TAG, task_ref,
};
use chrono::{NaiveDate, NaiveDateTime};
use rusqlite::types::{Value, ValueRef};
use rusqlite::{Connection, OptionalExtension, Params, Row, params, params_from_iter};
use std::collections::HashMap;
use std::path::Path;

const DATETIME_FMT: &str = "%Y-%m-%d %H:%M:%S";

fn dt_to_str(dt: NaiveDateTime) -> String {
    dt.format(DATETIME_FMT).to_string()
}

fn str_to_dt(s: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(s, DATETIME_FMT)
        .unwrap_or_else(|_| panic!("bad datetime in db: {s}"))
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
    fn values(&self) -> Vec<Value>;

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

    /// Brings the schema up to date. `PRAGMA user_version` records how many
    /// of [`Self::MIGRATIONS`] have run, so an up-to-date database costs one
    /// pragma read to open -- which matters to `iter serve`, opening one per
    /// request, and to shell completion, opening one per TAB. Each step runs
    /// in its own transaction together with bumping the version, so a step
    /// either lands whole or runs again next time.
    fn migrate(&self) -> Result<()> {
        let done: i64 = self
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))?;
        let done = usize::try_from(done).unwrap_or(0);
        for (version, step) in Self::MIGRATIONS.iter().enumerate().skip(done) {
            let tx = self.conn.unchecked_transaction()?;
            step(self)?;
            tx.execute_batch(&format!("PRAGMA user_version = {}", version + 1))?;
            tx.commit()?;
        }
        Ok(())
    }

    /// The schema's history, oldest first. Append a step; never edit one
    /// that has shipped.
    const MIGRATIONS: &[fn(&Db) -> Result<()>] = &[
        Self::baseline,
        Self::flags_to_tags,
        Self::drop_short_sessions,
    ];

    /// Everything from before versioning: idempotent, since a database
    /// that predates it could be at any point in that history.
    ///
    /// The `CREATE TABLE`s hold each table's original columns; every column
    /// added later is declared once, in [`Self::add_missing_columns`], which
    /// a fresh database runs too.
    fn baseline(&self) -> Result<()> {
        self.rename_legacy_tables()?;
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS boards (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                name            TEXT NOT NULL UNIQUE,
                description     TEXT NOT NULL DEFAULT ''
             );
             CREATE TABLE IF NOT EXISTS tags (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                name            TEXT NOT NULL UNIQUE,
                color           TEXT NOT NULL,
                description     TEXT NOT NULL DEFAULT ''
             );
             CREATE TABLE IF NOT EXISTS organizations (
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
                UNIQUE (project_id, name)
             );
             CREATE TABLE IF NOT EXISTS task_tags (
                task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                tag_id  INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
                PRIMARY KEY (task_id, tag_id)
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
        self.seed_tags()?;
        self.create_indexes();
        Ok(())
    }

    /// Urgent/important moved from two `tasks` columns onto the two
    /// built-in tags (see [`Priority`]): each set flag becomes a tag row, and
    /// the column goes. A database that never had the columns has nothing
    /// to move.
    fn flags_to_tags(&self) -> Result<()> {
        for (column, tag) in [("urgency", URGENT_TAG), ("importance", IMPORTANT_TAG)] {
            if !self.column_exists("tasks", column)? {
                continue;
            }
            self.conn.execute(
                &format!(
                    "INSERT OR IGNORE INTO task_tags (task_id, tag_id)
                     SELECT t.id, g.id FROM tasks t, tags g
                     WHERE g.name = ?1 AND t.{column} != 0"
                ),
                params![tag],
            )?;
            self.conn
                .execute_batch(&format!("ALTER TABLE tasks DROP COLUMN {column};"))?;
        }
        Ok(())
    }

    /// Closed sessions under [`Session::MIN_SECONDS`] were stored before
    /// closing learned to drop them; they go now. Measured in whole seconds
    /// (`strftime('%s')`): `julianday` differences carry float error that
    /// puts an exact minute at 59.99... Open sessions are left alone. A
    /// table without the two columns has no durations to measure.
    fn drop_short_sessions(&self) -> Result<()> {
        if !self.column_exists("sessions", "start")? || !self.column_exists("sessions", "end")? {
            return Ok(());
        }
        self.conn.execute(
            "DELETE FROM sessions
             WHERE end IS NOT NULL
               AND CAST(strftime('%s', end) AS INTEGER)
                 - CAST(strftime('%s', start) AS INTEGER) < ?1",
            params![Session::MIN_SECONDS],
        )?;
        Ok(())
    }

    /// Every task on board `id`'s projects as a [`Card`], ordered by
    /// project, then task. Three queries however big the board.
    pub fn cards(&self, board_id: i64) -> Result<Vec<Card>> {
        let priorities = self.priorities()?;
        Ok(self
            .tasks_in::<Board>(board_id)?
            .into_iter()
            .map(|(project, task)| Card {
                label: task_ref(&project, &task.name),
                priority: priorities.get(&task.id()).copied().unwrap_or_default(),
                task,
            })
            .collect())
    }

    /// Every task's [`Priority`], by task id -- one query for a listing of
    /// many. A task missing from the map has neither flag.
    pub fn priorities(&self) -> Result<HashMap<i64, Priority>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT tt.task_id, g.name FROM task_tags tt JOIN tags g ON g.id = tt.tag_id
             WHERE g.name IN (?1, ?2)",
        )?;
        let rows = stmt.query_map(params![URGENT_TAG, IMPORTANT_TAG], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut out: HashMap<i64, Priority> = HashMap::new();
        for row in rows {
            let (task_id, tag) = row?;
            let priority = out.entry(task_id).or_default();
            match tag == URGENT_TAG {
                true => priority.urgent = true,
                false => priority.important = true,
            }
        }
        Ok(out)
    }

    /// Makes `task_id`'s flags `priority`, by adding or removing the tags.
    #[cfg(any(test, feature = "web"))]
    pub fn set_priority(&self, task_id: i64, priority: Priority) -> Result<()> {
        for (tag, on) in [
            (URGENT_TAG, priority.urgent),
            (IMPORTANT_TAG, priority.important),
        ] {
            let sql = match on {
                true => {
                    "INSERT OR IGNORE INTO task_tags (task_id, tag_id)
                     SELECT ?1, id FROM tags WHERE name = ?2"
                }
                false => {
                    "DELETE FROM task_tags
                     WHERE task_id = ?1 AND tag_id = (SELECT id FROM tags WHERE name = ?2)"
                }
            };
            self.conn
                .prepare_cached(sql)?
                .execute(params![task_id, tag])?;
        }
        Ok(())
    }

    /// The two tags every database has: the ones the board views colour the
    /// Eisenhower flags with. `INSERT OR IGNORE` on the unique name, so a
    /// user's recolouring survives and re-running this is a no-op.
    fn seed_tags(&self) -> Result<()> {
        for tag in Tag::defaults() {
            self.conn.execute(
                "INSERT OR IGNORE INTO tags (name, color, description) VALUES (?1, ?2, ?3)",
                params![tag.name, tag.color, tag.description],
            )?;
        }
        Ok(())
    }

    /// Every tag attached to `task_id`, in name order.
    pub fn tags_for_task(&self, task_id: i64) -> Result<Vec<Tag>> {
        self.find_all(
            "WHERE id IN (SELECT tag_id FROM task_tags WHERE task_id = ?1) ORDER BY name",
            params![task_id],
        )
    }

    /// Replaces the tags on `task_id` with exactly `tag_ids`, in one
    /// transaction.
    pub fn set_task_tags(&self, task_id: i64, tag_ids: &[i64]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM task_tags WHERE task_id = ?1", params![task_id])?;
        let mut insert =
            tx.prepare("INSERT OR IGNORE INTO task_tags (task_id, tag_id) VALUES (?1, ?2)")?;
        for tag_id in tag_ids {
            insert.execute(params![task_id, tag_id])?;
        }
        drop(insert);
        tx.commit()?;
        Ok(())
    }

    /// The indexes the lookups above would otherwise scan whole tables for.
    ///
    /// Without `idx_sessions_task_start`, every `WHERE task_id = ?` on
    /// `sessions` is a full table scan, and a report walks one per task --
    /// an organization with 300 tasks scans the whole table 300 times.
    /// `start` rides along so the date-range predicate is answered from the
    /// index too, and so each task's sessions come back already ordered.
    ///
    /// Failures are deliberately swallowed. An index is an optimisation,
    /// never a correctness requirement, and it names columns that a
    /// database old enough may not have yet (the pre-rename `records`
    /// table became `sessions` without necessarily bringing `start` with
    /// it). Refusing to open the database over a missing index would turn
    /// a slow read into no read at all; instead the index appears on the
    /// first run where the schema can take it.
    fn create_indexes(&self) {
        let _ = self.conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_sessions_task_start
                ON sessions(task_id, start);",
        );
        let _ = self.conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_projects_organization
                ON projects(organization_id);",
        );
        let _ = self.conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_projects_board
                ON projects(board_id);",
        );
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
    /// already allowed to be in. `board_id` follows the same rule: a board
    /// is a container, so deleting one just unbinds its projects.
    fn add_missing_columns(&self) -> Result<()> {
        const ADDED: [(&str, &str, &str); 10] = [
            ("tasks", "matrix_placed", "INTEGER NOT NULL DEFAULT 0"),
            ("tasks", "start_time", "TEXT"),
            ("tasks", "duration", "INTEGER"),
            (
                "projects",
                "organization_id",
                "INTEGER REFERENCES organizations(id) ON DELETE SET NULL",
            ),
            ("tasks", "branch_prefix", "TEXT NOT NULL DEFAULT ''"),
            ("projects", "github_project", "TEXT NOT NULL DEFAULT ''"),
            (
                "organizations",
                "github_project",
                "TEXT NOT NULL DEFAULT ''",
            ),
            ("projects", "default_branch", "TEXT NOT NULL DEFAULT 'main'"),
            (
                "organizations",
                "default_branch",
                "TEXT NOT NULL DEFAULT 'main'",
            ),
            (
                "projects",
                "board_id",
                "INTEGER REFERENCES boards(id) ON DELETE SET NULL",
            ),
        ];
        for (table, column, decl) in ADDED {
            if !self.column_exists(table, column)? {
                self.conn
                    .execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {column} {decl};"))?;
            }
        }
        Ok(())
    }

    /// Asked the same way as [`Self::table_exists`] -- a single row that
    /// either comes back or doesn't, rather than every column name
    /// collected into a `Vec` to be scanned afterwards.
    fn column_exists(&self, table: &str, column: &str) -> Result<bool> {
        Ok(self
            .conn
            .query_row(
                &format!("SELECT 1 FROM pragma_table_info('{table}') WHERE name = ?1"),
                params![column],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
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
            .prepare_cached(&select_sql::<T>(tail))?
            .query_row(params, T::from_row)
            .optional()?)
    }

    /// Every row matching `tail` (a `WHERE`/`ORDER BY` clause).
    fn find_all<T: Table>(&self, tail: &str, params: impl Params) -> Result<Vec<T>> {
        let mut stmt = self.conn.prepare_cached(&select_sql::<T>(tail))?;
        let rows = stmt
            .query_map(params, T::from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    // ---- finders (lookups the generic CRUD surface doesn't cover) ---------

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

    /// The ids of the `T`s named `names`, all or nothing: an unknown name
    /// is an error before anything is written, so a typo can't half-apply
    /// a roster.
    pub fn ids_by_name<T: Named>(&self, names: &[String]) -> Result<Vec<i64>> {
        names
            .iter()
            .map(|n| Ok(self.resolve::<T>(n)?.id()))
            .collect()
    }

    /// Every project in group `id` (a board, an organization), in name order.
    pub fn projects_in<G: ProjectGroup>(&self, id: i64) -> Result<Vec<Project>> {
        self.find_all(
            &format!("WHERE {} = ?1 ORDER BY name", G::MEMBER_COLUMN),
            params![id],
        )
    }

    /// Makes `project_ids` exactly the projects in group `id`: the ones
    /// listed move in (out of whichever group they were in), the ones left
    /// off move out to none. Only membership changes -- a project keeps its
    /// own settings either way.
    pub fn set_projects<G: ProjectGroup>(&self, id: i64, project_ids: &[i64]) -> Result<()> {
        let column = G::MEMBER_COLUMN;
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            &format!("UPDATE projects SET {column} = NULL WHERE {column} = ?1"),
            params![id],
        )?;
        let mut join = tx.prepare(&format!("UPDATE projects SET {column} = ?1 WHERE id = ?2"))?;
        for project_id in project_ids {
            join.execute(params![id, project_id])?;
        }
        drop(join);
        tx.commit()?;
        Ok(())
    }

    /// Every task of every project in group `id`, paired with its project's
    /// name and ordered by project, then task. Two queries however many
    /// projects the group holds.
    pub fn tasks_in<G: ProjectGroup>(&self, id: i64) -> Result<Vec<(String, Task)>> {
        let names: HashMap<i64, String> = self
            .projects_in::<G>(id)?
            .into_iter()
            .map(|p| (p.id(), p.name))
            .collect();
        let tail = format!(
            "WHERE project_id IN (SELECT id FROM projects WHERE {} = ?1)",
            G::MEMBER_COLUMN
        );
        let mut tasks: Vec<(String, Task)> = self
            .find_all::<Task>(&tail, params![id])?
            .into_iter()
            .map(|t| (names.get(&t.project_id).cloned().unwrap_or_default(), t))
            .collect();
        tasks.sort_by(|(a, x), (b, y)| (a, &x.name).cmp(&(b, &y.name)));
        Ok(tasks)
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
        self.find_all("WHERE task_id = ?1 ORDER BY start", params![task_id])
    }

    /// The sessions of `task_id` that *start* within the range, oldest
    /// first -- the filter every report applies, answered by SQL instead of
    /// by loading a task's whole history and discarding most of it.
    pub fn sessions_for_task_in_range(
        &self,
        task_id: i64,
        from: Option<NaiveDate>,
        to: NaiveDate,
    ) -> Result<Vec<Session>> {
        let (lower, upper) = range_bounds(from, to);
        self.find_all(
            "WHERE task_id = ?1 AND start >= ?2 AND start < ?3 ORDER BY start",
            params![task_id, lower, upper],
        )
    }

    /// Every session in the range belonging to any task of `project_id`,
    /// oldest first. One query for the whole project rather than one per
    /// task; the caller groups by `task_id`, which each row carries.
    pub fn sessions_for_project_in_range(
        &self,
        project_id: i64,
        from: Option<NaiveDate>,
        to: NaiveDate,
    ) -> Result<Vec<Session>> {
        let (lower, upper) = range_bounds(from, to);
        // Written out rather than built by `select_sql`: the join puts an
        // `id` column on both sides, so the select list has to qualify it.
        let mut stmt = self.conn.prepare_cached(
            "SELECT s.id, s.task_id, s.start, s.end, s.message
             FROM sessions s JOIN tasks t ON t.id = s.task_id
             WHERE t.project_id = ?1 AND s.start >= ?2 AND s.start < ?3
             ORDER BY s.start",
        )?;
        let rows = stmt
            .query_map(params![project_id, lower, upper], Session::from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn open_session_for_task(&self, task_id: i64) -> Result<Option<Session>> {
        self.find_one("WHERE task_id = ?1 AND end IS NULL", params![task_id])
    }

    /// Every task as `(project name, task name)`, in `<project>/<task>`
    /// order, optionally narrowed to one project and/or a set of statuses.
    /// An empty `statuses` means every status, the same way `None` for
    /// `project` means every project.
    ///
    /// One join rather than a listing plus a query per project: this runs
    /// on every TAB keypress, where the per-project round trips were the
    /// dominant cost and every task's other columns were loaded only to be
    /// dropped.
    pub fn task_refs(
        &self,
        project: Option<&str>,
        statuses: &[TaskStatus],
    ) -> Result<Vec<(String, String)>> {
        let mut sql = String::from(
            "SELECT p.name, t.name FROM tasks t JOIN projects p ON p.id = t.project_id",
        );
        let mut clauses = Vec::new();
        if project.is_some() {
            clauses.push("p.name = ?".to_string());
        }
        if !statuses.is_empty() {
            let placeholders = vec!["?"; statuses.len()].join(", ");
            clauses.push(format!("t.status IN ({placeholders})"));
        }
        if !clauses.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&clauses.join(" AND "));
        }
        sql.push_str(" ORDER BY p.name, t.name");

        // Anonymous `?` bound positionally, so the filters compose without
        // each combination needing its own `query_map` arm.
        let mut values: Vec<Value> = Vec::new();
        values.extend(project.map(|name| Value::Text(name.to_string())));
        values.extend(statuses.iter().map(|s| Value::Text(s.as_str().to_string())));

        let mut stmt = self.conn.prepare_cached(&sql)?;
        let rows = stmt
            .query_map(params_from_iter(values), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Every distinct task name in the database, sorted -- what `task
    /// pull/push --task` completes, which takes a bare name rather than a
    /// `<project>/<task>` pair. SQL does the dedup and the ordering.
    pub fn task_names(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT DISTINCT name FROM tasks ORDER BY name")?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Every `T`'s name, in `T`'s listing order -- what `iter <entity>
    /// list` prints and what shell completion offers. A listing of names is
    /// a query, so it lives here rather than in either caller.
    pub fn names<T: Named>(&self) -> Result<Vec<String>> {
        Ok(self
            .list::<T>()?
            .into_iter()
            .map(|item| item.name().to_string())
            .collect())
    }

    // ---- generic CRUD, one implementation for every `Table` --------------

    /// Writes `item` as a new row and returns the id SQLite assigned it.
    pub fn insert<T: Table>(&self, item: &T) -> Result<i64> {
        self.conn
            .prepare_cached(&insert_sql::<T>())?
            .execute(params_from_iter(item.values()))?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Overwrites row `id` with `item`'s columns.
    pub fn update<T: Table>(&self, id: i64, item: &T) -> Result<()> {
        let mut values = item.values();
        values.push(id.into());
        self.conn
            .prepare_cached(&update_sql::<T>())?
            .execute(params_from_iter(values))?;
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
    pub fn list<T: Table>(&self) -> Result<Vec<T>> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Organization;
    use crate::models::Settings;

    fn day(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .unwrap_or_else(|e| panic!("bad test fixture date '{s}': {e}"))
    }

    fn dt(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M")
            .unwrap_or_else(|e| panic!("bad test fixture datetime '{s}': {e}"))
    }

    /// Runs every migration again, as on a database from before versioning
    /// -- what the tests that hand-build an old schema need.
    fn remigrate(db: &Db) -> Result<()> {
        db.conn.execute_batch("PRAGMA user_version = 0")?;
        db.migrate()
    }

    /// A fresh, empty, migrated database held entirely in memory.
    fn db() -> Db {
        Db::open(":memory:").expect("in-memory database opens")
    }

    fn project(name: &str) -> Project {
        Project {
            id: None,
            organization_id: None,
            board_id: None,
            name: name.to_string(),
            description: "notes".to_string(),
            base_path: format!("/tmp/{name}"),
            github: true,
            tmux: false,
            auto_branch: true,
            branch_template: "fix/{task}".to_string(),
            default_branch: "main".to_string(),
            github_project: "Roadmap".to_string(),
        }
    }

    fn insert_project(db: &Db, name: &str) -> i64 {
        db.insert(&project(name)).expect("project inserts")
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
            matrix_placed: false,
            start_time: None,
            duration: None,
        };
        db.insert(&task).expect("task inserts")
    }

    /// A database from before urgent/important were tags carries them as
    /// two columns; migrating turns each set flag into the tag and drops
    /// the columns, leaving tasks with neither flag untagged.
    #[test]
    fn migrating_moves_the_flag_columns_onto_the_builtin_tags() {
        let db = db();
        let project = insert_project(&db, "p");
        let (both, urgent, neither) = (
            insert_task(&db, project, "both"),
            insert_task(&db, project, "urgent"),
            insert_task(&db, project, "neither"),
        );
        db.conn
            .execute_batch(&format!(
                "ALTER TABLE tasks ADD COLUMN urgency INTEGER NOT NULL DEFAULT 0;
                 ALTER TABLE tasks ADD COLUMN importance INTEGER NOT NULL DEFAULT 0;
                 UPDATE tasks SET urgency = 1 WHERE id IN ({both}, {urgent});
                 UPDATE tasks SET importance = 1 WHERE id = {both};"
            ))
            .expect("legacy columns");

        remigrate(&db).expect("migrates");

        assert!(!db.column_exists("tasks", "urgency").expect("check"));
        assert!(!db.column_exists("tasks", "importance").expect("check"));
        let flags = db.priorities().expect("priorities");
        let of = |id| flags.get(&id).copied().unwrap_or_default();
        assert_eq!(
            of(both),
            Priority {
                urgent: true,
                important: true
            }
        );
        assert_eq!(
            of(urgent),
            Priority {
                urgent: true,
                important: false
            }
        );
        assert_eq!(of(neither), Priority::default());
    }

    /// Setting a priority adds and removes only the two built-in tags; a
    /// task's other tags are left alone, and they show up on its cards.
    #[test]
    fn set_priority_toggles_only_the_builtin_tags() {
        let db = db();
        let work = db.insert(&board("work")).expect("board");
        let project = insert_project(&db, "p");
        db.set_projects::<Board>(work, &[project]).expect("roster");
        let task = insert_task(&db, project, "t");
        let other = db
            .insert(&Tag {
                name: "home".to_string(),
                ..Tag::template()
            })
            .expect("tag");
        db.set_task_tags(task, &[other]).expect("tags");

        let flags = Priority {
            urgent: true,
            important: false,
        };
        db.set_priority(task, flags).expect("set");
        let cards = db.cards(work).expect("cards");
        assert_eq!((cards[0].label.as_str(), cards[0].priority), ("p/t", flags));

        db.set_priority(task, Priority::default()).expect("clear");
        let names: Vec<String> = db
            .tags_for_task(task)
            .expect("tags")
            .into_iter()
            .map(|t| t.name)
            .collect();
        assert_eq!(names, ["home"]);
    }

    /// A migrated database records its version, and opening it again runs
    /// nothing: a step that would fail if re-run (here, a table the baseline
    /// would recreate is gone) shows it was skipped.
    #[test]
    fn an_up_to_date_database_skips_every_migration() {
        let db = db();
        let version: i64 = db
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("version");
        assert_eq!(version, Db::MIGRATIONS.len() as i64);

        db.conn.execute_batch("DROP TABLE boards").expect("drop");
        db.migrate().expect("nothing to do");
        assert!(!db.table_exists("boards").expect("check"));
        remigrate(&db).expect("from scratch");
        assert!(db.table_exists("boards").expect("check"));
    }

    /// The date window is half-open on the upper end, so the boundaries
    /// are worth pinning: a session starting at 00:00:00 on `from` is in,
    /// one at 23:59:59 on `to` is in, and the first instant of the day
    /// after `to` is out. This is the filter every report applies, and it
    /// is now SQL's job rather than a Rust-side pass.
    #[test]
    fn a_range_query_includes_both_boundary_days_whole() {
        let db = db();
        let project = insert_project(&db, "proj");
        let task = insert_task(&db, project, "t");
        for start in [
            "2025-03-02 23:59:59", // day before `from` -- out
            "2025-03-03 00:00:00", // first instant of `from` -- in
            "2025-03-04 12:00:00", // inside -- in
            "2025-03-05 23:59:59", // last instant of `to` -- in
            "2025-03-06 00:00:00", // first instant after `to` -- out
        ] {
            let session = Session {
                id: None,
                task_id: task,
                start: dt(&start[..16]),
                end: None,
                message: None,
            };
            db.insert(&session).expect("session inserts");
        }

        let starts: Vec<String> = db
            .sessions_for_task_in_range(task, Some(day("2025-03-03")), day("2025-03-05"))
            .expect("range query")
            .iter()
            .map(|s| s.start.to_string())
            .collect();
        assert_eq!(
            starts,
            vec![
                "2025-03-03 00:00:00".to_string(),
                "2025-03-04 12:00:00".to_string(),
                "2025-03-05 23:59:00".to_string(),
            ]
        );
    }

    /// An absent `from` reaches back to the beginning of time rather than
    /// to some arbitrary epoch.
    #[test]
    fn an_open_start_reaches_back_past_any_stored_session() {
        let db = db();
        let project = insert_project(&db, "proj");
        let task = insert_task(&db, project, "t");
        let session = Session {
            id: None,
            task_id: task,
            start: dt("1970-01-02 03:04"),
            end: None,
            message: None,
        };
        db.insert(&session).expect("session inserts");

        assert_eq!(
            db.sessions_for_task_in_range(task, None, day("2025-03-05"))
                .expect("range query")
                .len(),
            1
        );
    }

    /// The project-wide query is the same window, across every task, and
    /// it is what a project or organization report is built from.
    #[test]
    fn a_project_range_query_spans_its_tasks_in_time_order() {
        let db = db();
        let project = insert_project(&db, "proj");
        let other = insert_project(&db, "other");
        let a = insert_task(&db, project, "a");
        let b = insert_task(&db, project, "b");
        let elsewhere = insert_task(&db, other, "c");
        for (task, start) in [
            (b, "2025-03-04 09:00"),
            (a, "2025-03-03 09:00"),
            (elsewhere, "2025-03-03 10:00"), // another project -- excluded
            (a, "2025-03-09 09:00"),         // outside the range -- excluded
        ] {
            let session = Session {
                id: None,
                task_id: task,
                start: dt(start),
                end: None,
                message: None,
            };
            db.insert(&session).expect("session inserts");
        }

        let found = db
            .sessions_for_project_in_range(project, Some(day("2025-03-03")), day("2025-03-05"))
            .expect("range query");
        assert_eq!(
            found
                .iter()
                .map(|s| (s.task_id, s.start.to_string()))
                .collect::<Vec<_>>(),
            vec![
                (a, "2025-03-03 09:00:00".to_string()),
                (b, "2025-03-04 09:00:00".to_string()),
            ]
        );
    }

    /// `task list` narrows by project and by a set of statuses at once;
    /// both filters reach SQL, and an empty set means "every status".
    #[test]
    fn task_refs_narrows_by_project_and_status_set() {
        let db = db();
        let alpha = insert_project(&db, "alpha");
        let beta = insert_project(&db, "beta");
        insert_task(&db, alpha, "wip one"); // inserted as `wip`
        insert_task(&db, beta, "wip two");
        for (project, name, status) in [
            (alpha, "queued", TaskStatus::Queue),
            (alpha, "finished", TaskStatus::Done),
        ] {
            let task = Task {
                id: None,
                project_id: project,
                name: name.to_string(),
                description: String::new(),
                github_issue: None,
                status,
                branch_prefix: String::new(),
                matrix_placed: false,
                start_time: None,
                duration: None,
            };
            db.insert(&task).expect("task inserts");
        }

        assert_eq!(
            db.task_refs(Some("alpha"), &[]).expect("refs"),
            vec![
                ("alpha".to_string(), "finished".to_string()),
                ("alpha".to_string(), "queued".to_string()),
                ("alpha".to_string(), "wip one".to_string()),
            ]
        );
        assert_eq!(
            db.task_refs(None, &[TaskStatus::Queue, TaskStatus::Wip])
                .expect("refs"),
            vec![
                ("alpha".to_string(), "queued".to_string()),
                ("alpha".to_string(), "wip one".to_string()),
                ("beta".to_string(), "wip two".to_string()),
            ]
        );
        assert_eq!(
            db.task_refs(Some("beta"), &[TaskStatus::Queue])
                .expect("refs"),
            []
        );
    }

    /// The completion lookups are one join rather than a query per
    /// project, so what they return is worth pinning: every task, paired
    /// with its own project, ordered the way completion offers it.
    #[test]
    fn task_refs_pairs_every_task_with_its_project() {
        let db = db();
        let beta = insert_project(&db, "beta");
        let alpha = insert_project(&db, "alpha");
        insert_task(&db, beta, "ship it");
        insert_task(&db, alpha, "write docs");
        insert_task(&db, alpha, "another");

        assert_eq!(
            db.task_refs(None, &[]).expect("task refs"),
            vec![
                ("alpha".to_string(), "another".to_string()),
                ("alpha".to_string(), "write docs".to_string()),
                ("beta".to_string(), "ship it".to_string()),
            ]
        );
    }

    /// `session new` only completes tasks that haven't been started, so the
    /// status filter has to reach SQL rather than being applied after every
    /// task in the database was loaded.
    #[test]
    fn task_refs_can_narrow_to_one_status() {
        let db = db();
        let project = insert_project(&db, "proj");
        insert_task(&db, project, "in progress"); // inserted as `wip`
        let queued = Task {
            id: None,
            project_id: project,
            name: "queued".to_string(),
            description: String::new(),
            github_issue: None,
            status: TaskStatus::Queue,
            branch_prefix: String::new(),
            matrix_placed: false,
            start_time: None,
            duration: None,
        };
        db.insert(&queued).expect("task inserts");

        assert_eq!(
            db.task_refs(None, &[TaskStatus::Queue]).expect("task refs"),
            vec![("proj".to_string(), "queued".to_string())]
        );
        assert_eq!(
            db.task_refs(None, &[TaskStatus::Done]).expect("task refs"),
            []
        );
    }

    /// `--task` takes a bare name, so the same name under two projects is
    /// one candidate, not two -- deduped and ordered by SQL.
    #[test]
    fn task_names_are_distinct_and_sorted() {
        let db = db();
        let one = insert_project(&db, "one");
        let two = insert_project(&db, "two");
        insert_task(&db, one, "shared");
        insert_task(&db, two, "shared");
        insert_task(&db, one, "alone");

        assert_eq!(
            db.task_names().expect("task names"),
            vec!["alone".to_string(), "shared".to_string()]
        );
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
             branch_template, default_branch, github_project) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
        );
        assert_eq!(
            select_sql::<Organization>("WHERE name = ?1"),
            "SELECT id, name, description, github, tmux, auto_branch, branch_template, \
             default_branch, github_project FROM organizations WHERE name = ?1"
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
        check(&Organization::template(&Settings::default()));
        check(&Task {
            id: None,
            project_id: 1,
            name: "t".to_string(),
            description: String::new(),
            github_issue: None,
            status: TaskStatus::Queue,
            branch_prefix: String::new(),
            matrix_placed: false,
            start_time: None,
            duration: None,
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
        let loaded = db
            .get::<Project>(id)
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
        assert_eq!(loaded.github_project, "Roadmap");
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
                    board_id        INTEGER REFERENCES boards(id) ON DELETE SET NULL,
                    name            TEXT NOT NULL UNIQUE,
                    description     TEXT NOT NULL DEFAULT '',
                    base_path       TEXT NOT NULL,
                    github          INTEGER NOT NULL DEFAULT 0,
                    tmux            INTEGER NOT NULL DEFAULT 1,
                    auto_branch     INTEGER NOT NULL DEFAULT 1,
                    branch_template TEXT NOT NULL DEFAULT 'feat/{task}',
                    default_branch  TEXT NOT NULL DEFAULT 'main',
                    github_project  TEXT NOT NULL DEFAULT '',
                    auto_issue      INTEGER NOT NULL DEFAULT 0
                 );",
            )
            .expect("drifted schema is created");

        let id = insert_project(&db, "alpha");
        db.update(id, &project("renamed")).expect("update succeeds");

        let loaded = db
            .get::<Project>(id)
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
        remigrate(&db).expect("migrating adds the column");

        let project_id = insert_project(&db, "alpha");
        let id = insert_task(&db, project_id, "build it");
        let loaded = db
            .get::<Task>(id)
            .expect("get succeeds")
            .expect("the row just inserted exists");
        assert_eq!(loaded.branch_prefix, "fix/");
    }

    /// The same guarantee for the settings that land on two tables at once
    /// -- `github_project` and `default_branch`. A database that predates
    /// one has to gain the column on `projects` *and* on `organizations`,
    /// or the setting a project inherits has nowhere to be read from.
    #[test]
    fn pre_existing_tables_gain_the_columns_added_since() {
        let db = db();
        db.conn
            .execute_batch(
                "DROP TABLE projects;
                 DROP TABLE organizations;
                 CREATE TABLE organizations (
                    id              INTEGER PRIMARY KEY AUTOINCREMENT,
                    name            TEXT NOT NULL UNIQUE,
                    description     TEXT NOT NULL DEFAULT '',
                    github          INTEGER NOT NULL DEFAULT 0,
                    tmux            INTEGER NOT NULL DEFAULT 1,
                    auto_branch     INTEGER NOT NULL DEFAULT 1,
                    branch_template TEXT NOT NULL DEFAULT 'feat/{task}'
                 );
                 CREATE TABLE projects (
                    id              INTEGER PRIMARY KEY AUTOINCREMENT,
                    organization_id INTEGER REFERENCES organizations(id) ON DELETE SET NULL,
                    board_id        INTEGER REFERENCES boards(id) ON DELETE SET NULL,
                    name            TEXT NOT NULL UNIQUE,
                    description     TEXT NOT NULL DEFAULT '',
                    base_path       TEXT NOT NULL,
                    github          INTEGER NOT NULL DEFAULT 0,
                    tmux            INTEGER NOT NULL DEFAULT 1,
                    auto_branch     INTEGER NOT NULL DEFAULT 1,
                    branch_template TEXT NOT NULL DEFAULT 'feat/{task}'
                 );",
            )
            .expect("the old schema is created");
        remigrate(&db).expect("migrating adds the columns");

        let id = insert_project(&db, "alpha");
        let loaded = db
            .get::<Project>(id)
            .expect("get succeeds")
            .expect("the row just inserted exists");
        assert_eq!(loaded.github_project, "Roadmap");
        assert_eq!(loaded.default_branch, "main");

        let organization_id = insert_organization(&db, "acme");
        let loaded = db
            .get::<Organization>(organization_id)
            .expect("get succeeds")
            .expect("the row just inserted exists");
        assert_eq!(loaded.github_project, "Acme Roadmap");
        assert_eq!(loaded.default_branch, "dev");
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
        db.update(id, &changed).expect("update succeeds");

        let loaded = db
            .get::<Project>(id)
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

        let names: Vec<String> = db
            .list::<Project>()
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
            db.find_by_name::<Project>("alpha")
                .expect("lookup succeeds")
                .is_some()
        );
        assert!(
            db.find_by_name::<Project>("nope")
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
            matrix_placed: false,
            start_time: None,
            duration: None,
        };
        let id = db.insert(&task).expect("task inserts");
        let loaded = db
            .get::<Task>(id)
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

        db.delete::<Project>(project_id).expect("delete succeeds");
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
        db.insert(&config).expect("config inserts");

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
        db.insert(&closed).expect("closed session inserts");
        let open_id = db.insert(&open).expect("open session inserts");

        let found = db
            .open_session_for_task(task_id)
            .expect("lookup succeeds")
            .expect("there is an open session");
        assert_eq!(found.id, Some(open_id));
        assert!(found.is_ongoing());
        assert_eq!(found.start, dt("2026-09-01 11:00"));
    }

    /// The migration drops closed sessions under a minute -- 59 seconds
    /// goes, exactly 60 stays -- and never an open one, however recent.
    #[test]
    fn short_closed_sessions_are_dropped_by_the_migration() {
        let db = db();
        let project_id = insert_project(&db, "alpha");
        let task_id = insert_task(&db, project_id, "build it");
        let at = |h, m, s| day("2026-09-01").and_hms_opt(h, m, s).expect("valid time");
        let session = |start, end| Session {
            id: None,
            task_id,
            start,
            end,
            message: None,
        };
        db.insert(&session(at(9, 0, 0), Some(at(9, 0, 59))))
            .expect("59s");
        db.insert(&session(at(10, 0, 0), Some(at(10, 0, 0))))
            .expect("0s");
        let minute = db
            .insert(&session(at(11, 0, 0), Some(at(11, 1, 0))))
            .expect("60s");
        let open = db.insert(&session(at(12, 0, 0), None)).expect("open");

        db.drop_short_sessions().expect("migration runs");

        let left: Vec<_> = db
            .sessions_for_task(task_id)
            .expect("lookup")
            .into_iter()
            .map(|s| s.id)
            .collect();
        assert_eq!(left, [Some(minute), Some(open)]);
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
        let id = db.insert(&session).expect("session inserts");

        let mut closed = session;
        closed.end = Some(dt("2026-09-01 10:30"));
        closed.message = Some("wrapped up".to_string());
        db.update(id, &closed).expect("update succeeds");

        let loaded = db
            .get::<Session>(id)
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
            db.insert(&session).expect("session inserts");
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
        let mut organization = Organization::template(&Settings::default());
        organization.name = name.to_string();
        organization.github = true;
        organization.tmux = false;
        organization.branch_template = "chore/{task}".to_string();
        organization.github_project = "Acme Roadmap".to_string();
        organization.default_branch = "dev".to_string();
        db.insert(&organization).expect("organization inserts")
    }

    #[test]
    fn organization_round_trips_its_downstream_defaults() {
        let db = db();
        let id = insert_organization(&db, "acme");
        let loaded = db
            .get::<Organization>(id)
            .expect("get succeeds")
            .expect("the row just inserted exists");
        assert_eq!(loaded.name, "acme");
        assert!(loaded.github);
        assert!(!loaded.tmux);
        assert!(loaded.auto_branch);
        assert_eq!(loaded.branch_template, "chore/{task}");
        assert_eq!(loaded.github_project, "Acme Roadmap");
    }

    /// Setting the roster moves listed projects in -- even out of another
    /// organization -- and drops unlisted ones to no organization, without
    /// touching anyone else's members.
    #[test]
    fn setting_an_organizations_projects_moves_them_in_and_out() {
        let db = db();
        let acme = insert_organization(&db, "acme");
        let other = insert_organization(&db, "other");
        let kept = insert_project(&db, "kept");
        let dropped = insert_project(&db, "dropped");
        let stolen = insert_project(&db, "stolen");
        let bystander = insert_project(&db, "bystander");
        db.set_projects::<Organization>(acme, &[kept, dropped])
            .unwrap();
        db.set_projects::<Organization>(other, &[stolen, bystander])
            .unwrap();

        db.set_projects::<Organization>(acme, &[kept, stolen])
            .unwrap();

        let names = |org| -> Vec<String> {
            db.projects_in::<Organization>(org)
                .unwrap()
                .into_iter()
                .map(|p| p.name)
                .collect()
        };
        assert_eq!(names(acme), ["kept", "stolen"]);
        assert_eq!(names(other), ["bystander"]);
        assert_eq!(
            db.get::<Project>(dropped).unwrap().unwrap().organization_id,
            None
        );
    }

    #[test]
    fn a_project_may_belong_to_an_organization_or_to_none() {
        let db = db();
        let organization = insert_organization(&db, "acme");

        let mut member = project("member");
        member.organization_id = Some(organization);
        let member_id = db.insert(&member).expect("project inserts");
        let loner_id = insert_project(&db, "loner");

        assert_eq!(
            db.get::<Project>(member_id)
                .expect("get succeeds")
                .expect("row exists")
                .organization_id,
            Some(organization)
        );
        assert_eq!(
            db.get::<Project>(loner_id)
                .expect("get succeeds")
                .expect("row exists")
                .organization_id,
            None,
            "a project without an organization must round-trip as NULL"
        );

        let roster: Vec<String> = db
            .projects_in::<Organization>(organization)
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
        let project_id = db.insert(&member).expect("project inserts");
        let task_id = insert_task(&db, project_id, "build it");

        db.delete::<Organization>(organization)
            .expect("delete succeeds");

        let loaded = db
            .get::<Project>(project_id)
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

        remigrate(&db).expect("migration succeeds");

        assert!(db.column_exists("projects", "organization_id").unwrap());
        // ...and the column is usable, defaulting to "no organization".
        let id = insert_project(&db, "legacy");
        assert_eq!(
            db.get::<Project>(id)
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

        remigrate(&db).expect("migration succeeds");

        assert!(db.table_exists("session_configs").expect("check succeeds"));
        assert!(db.table_exists("sessions").expect("check succeeds"));
        assert!(!db.table_exists("records").expect("check succeeds"));
    }

    #[test]
    fn resolve_names_the_kind_it_could_not_find() {
        let db = db();
        insert_project(&db, "alpha");
        assert_eq!(db.resolve::<Project>("alpha").expect("found").name, "alpha");
        let missing = db.resolve::<Board>("nope").expect_err("no such board");
        assert_eq!(missing.to_string(), "no such board 'nope'");
    }

    /// One unknown name fails the whole lookup, so a roster is never
    /// half-applied.
    #[test]
    fn ids_by_name_is_all_or_nothing() {
        let db = db();
        let a = insert_project(&db, "a");
        let b = insert_project(&db, "b");
        let names = |ns: &[&str]| ns.iter().map(|n| n.to_string()).collect::<Vec<_>>();
        assert_eq!(
            db.ids_by_name::<Project>(&names(&["b", "a"])).expect("ids"),
            [b, a]
        );
        assert!(db.ids_by_name::<Project>(&names(&["a", "typo"])).is_err());
        assert_eq!(
            db.ids_by_name::<Tag>(&names(&["Urgent"]))
                .expect("seeded")
                .len(),
            1
        );
    }

    /// Board and organization membership are independent columns driven by
    /// the same generic code: moving a project on one leaves the other be.
    #[test]
    fn set_projects_touches_only_its_own_group_column() {
        let db = db();
        let work = db.insert(&board("work")).expect("insert board");
        let acme = insert_organization(&db, "acme");
        let (a, b) = (insert_project(&db, "a"), insert_project(&db, "b"));
        db.set_projects::<Organization>(acme, &[a, b])
            .expect("org roster");
        db.set_projects::<Board>(work, &[a]).expect("board roster");
        db.set_projects::<Board>(work, &[b]).expect("board roster");

        let names = |ps: Vec<Project>| ps.into_iter().map(|p| p.name).collect::<Vec<_>>();
        assert_eq!(names(db.projects_in::<Board>(work).expect("board")), ["b"]);
        assert_eq!(
            names(db.projects_in::<Organization>(acme).expect("org")),
            ["a", "b"]
        );
    }

    /// The tasks of a group come back with their project's name, ordered by
    /// project then task, and only from that group's projects.
    #[test]
    fn tasks_in_a_group_pair_each_task_with_its_project() {
        let db = db();
        let work = db.insert(&board("work")).expect("insert board");
        let (b, a, off) = (
            insert_project(&db, "b"),
            insert_project(&db, "a"),
            insert_project(&db, "off"),
        );
        db.set_projects::<Board>(work, &[a, b]).expect("roster");
        for (project, name) in [(b, "y"), (a, "z"), (a, "x"), (off, "w")] {
            insert_task(&db, project, name);
        }
        let got: Vec<(String, String)> = db
            .tasks_in::<Board>(work)
            .expect("query")
            .into_iter()
            .map(|(project, task)| (project, task.name))
            .collect();
        let want =
            [("a", "x"), ("a", "z"), ("b", "y")].map(|(p, t)| (p.to_string(), t.to_string()));
        assert_eq!(got, want);
        assert!(db.tasks_in::<Organization>(1).expect("query").is_empty());
    }

    fn board(name: &str) -> crate::models::Board {
        crate::models::Board {
            id: None,
            name: name.to_string(),
            description: String::new(),
        }
    }

    /// Binding is the whole point of a board: a project put on one shows
    /// up in `projects_in::<Board>`, and a project on another board or none
    /// does not.
    #[test]
    fn projects_in_a_board_are_only_the_projects_bound_to_it() {
        let db = db();
        let work = db.insert(&board("work")).expect("insert board");
        let home = db.insert(&board("home")).expect("insert board");
        let (a, b, c) = (
            insert_project(&db, "a"),
            insert_project(&db, "b"),
            insert_project(&db, "c"),
        );
        for (id, board_id) in [(a, Some(work)), (b, Some(home)), (c, None)] {
            let mut p = db.get::<Project>(id).expect("get").expect("exists");
            p.board_id = board_id;
            db.update(id, &p).expect("update");
        }

        let names: Vec<String> = db
            .projects_in::<Board>(work)
            .expect("query")
            .into_iter()
            .map(|p| p.name)
            .collect();
        assert_eq!(names, ["a"]);
    }

    /// Every database starts with the two tags the board views colour the
    /// Eisenhower flags with, and re-running the migration neither
    /// duplicates them nor undoes a recolouring.
    #[test]
    fn urgent_and_important_tags_are_seeded_once() {
        let db = db();
        let mut urgent = db
            .find_by_name::<Tag>("Urgent")
            .expect("find")
            .expect("seeded");
        let important = db
            .find_by_name::<Tag>("Important")
            .expect("find")
            .expect("seeded");
        assert_eq!(
            (urgent.color.as_str(), important.color.as_str()),
            ("#facc15", "#3b82f6")
        );

        urgent.color = "#ff0000".into();
        db.update(urgent.id(), &urgent).expect("recolor");
        db.seed_tags().expect("reseed");
        assert_eq!(db.list::<Tag>().expect("list").len(), 2);
        let urgent = db
            .find_by_name::<Tag>("Urgent")
            .expect("find")
            .expect("seeded");
        assert_eq!(urgent.color, "#ff0000");
    }

    /// Tags are shared: setting replaces, and deleting a task or a tag
    /// drops only the link.
    #[cfg(feature = "web")]
    #[test]
    fn task_tags_replace_and_cascade() {
        let db = db();
        let project = insert_project(&db, "a");
        let task = insert_task(&db, project, "t");
        let urgent = db
            .find_by_name::<Tag>("Urgent")
            .expect("find")
            .expect("seeded")
            .id();
        let important = db
            .find_by_name::<Tag>("Important")
            .expect("find")
            .expect("seeded")
            .id();

        db.set_task_tags(task, &[urgent, important]).expect("set");
        assert_eq!(db.tags_for_task(task).expect("tags").len(), 2);
        db.set_task_tags(task, &[important]).expect("replace");
        let tags = db.tags_for_task(task).expect("tags");
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "Important");

        db.delete::<Tag>(important).expect("delete tag");
        assert!(db.tags_for_task(task).expect("tags").is_empty());
        assert!(db.find_by_name::<Tag>("Urgent").expect("find").is_some());
    }

    /// A board is a container, not an owner: deleting one must unbind its
    /// projects rather than delete them.
    #[test]
    fn deleting_a_board_keeps_its_projects_and_unbinds_them() {
        let db = db();
        let board_id = db.insert(&board("work")).expect("insert board");
        let id = insert_project(&db, "a");
        let mut p = db.get::<Project>(id).expect("get").expect("exists");
        p.board_id = Some(board_id);
        db.update(id, &p).expect("update");

        db.delete::<crate::models::Board>(board_id).expect("delete");

        let p = db.get::<Project>(id).expect("get").expect("project kept");
        assert_eq!(p.board_id, None);
    }

    /// Databases created before boards existed have a `projects` table
    /// without `board_id`; opening one must add it.
    #[test]
    fn migration_adds_board_id_to_a_projects_table_that_lacks_it() {
        let dir = std::env::temp_dir().join(format!("iter-board-mig-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("iter.db");
        let _ = std::fs::remove_file(&path);
        {
            let conn = Connection::open(&path).expect("open");
            conn.execute_batch(
                "CREATE TABLE projects (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL UNIQUE,
                    base_path TEXT NOT NULL
                 );
                 INSERT INTO projects (name, base_path) VALUES ('old', '/tmp/old');",
            )
            .expect("legacy schema");
        }
        let db = Db::open(path.to_str().expect("utf8 path")).expect("migrates");
        assert!(db.column_exists("projects", "board_id").expect("check"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn task_schedule_round_trips_and_migrates_onto_an_old_tasks_table() {
        let db = db();
        let project_id = insert_project(&db, "alpha");
        let mut task = Task::template(project_id, String::new(), TaskStatus::Queue);
        task.name = "scheduled".to_string();
        task.start_time = Some(dt("2026-09-28 09:30"));
        task.duration = Some(Duration(90));
        db.insert(&task).expect("insert");
        let found = db
            .find_task(project_id, "scheduled")
            .expect("lookup")
            .expect("exists");
        assert_eq!(found.start_time, Some(dt("2026-09-28 09:30")));
        assert_eq!(found.duration, Some(Duration(90)));

        let dir = std::env::temp_dir().join(format!("iter-task-mig-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("iter.db");
        let _ = std::fs::remove_file(&path);
        {
            let conn = Connection::open(&path).expect("open");
            conn.execute_batch(
                "CREATE TABLE tasks (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    project_id INTEGER NOT NULL,
                    name TEXT NOT NULL
                 );",
            )
            .expect("legacy schema");
        }
        let db = Db::open(path.to_str().expect("utf8 path")).expect("migrates");
        for column in ["start_time", "duration"] {
            assert!(
                db.column_exists("tasks", column).expect("check"),
                "{column}"
            );
        }
        std::fs::remove_dir_all(&dir).ok();
    }
}
