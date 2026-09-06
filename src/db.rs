use crate::error::Result;
use crate::models::{Project, Record, Session, Task, TaskStatus};
use chrono::NaiveDateTime;
use rusqlite::{Connection, OptionalExtension, Row, params};
use std::path::Path;

pub const DB_FILE: &str = "/home/local/.config/programmini/buff/iter.db";

const DATETIME_FMT: &str = "%Y-%m-%d %H:%M:%S";

pub fn dt_to_str(dt: NaiveDateTime) -> String {
    dt.format(DATETIME_FMT).to_string()
}

pub fn str_to_dt(s: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(s, DATETIME_FMT)
        .unwrap_or_else(|_| panic!("bad datetime in db: {s}"))
}

/// Generic storage surface, implemented once per entity below (`Project`,
/// `Task`, `Session`, `Record`) against the one `Db`/SQLite backend.
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
    pub fn open(path: &str) -> Result<Self> {
        if let Some(parent) = Path::new(path).parent() {
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
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS projects (
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
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                project_id   INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                name         TEXT NOT NULL,
                description  TEXT NOT NULL DEFAULT '',
                github_issue INTEGER,
                status       TEXT NOT NULL DEFAULT 'queue' CHECK (status IN ('queue', 'wip', 'done')),
                UNIQUE (project_id, name)
             );
             CREATE TABLE IF NOT EXISTS sessions (
                id                INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id           INTEGER NOT NULL UNIQUE REFERENCES tasks(id) ON DELETE CASCADE,
                tmux_session_name TEXT,
                github_branch     TEXT,
                worktree_path     TEXT
             );
             CREATE TABLE IF NOT EXISTS records (
                id      INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                start   TEXT NOT NULL,
                end     TEXT,
                message TEXT
             );",
        )?;
        Ok(())
    }

    // ---- finder helpers (not part of the generic CRUD surface) -----------

    pub fn find_project_by_name(&self, name: &str) -> Result<Option<Project>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, name, description, base_path, github, tmux, auto_branch, branch_template
                 FROM projects WHERE name = ?1",
                params![name],
                Self::row_to_project,
            )
            .optional()?)
    }

    pub fn find_task(&self, project_id: i64, task_name: &str) -> Result<Option<Task>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, project_id, name, description, github_issue, status
                 FROM tasks WHERE project_id = ?1 AND name = ?2",
                params![project_id, task_name],
                Self::row_to_task,
            )
            .optional()?)
    }

    pub fn tasks_for_project(&self, project_id: i64) -> Result<Vec<Task>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project_id, name, description, github_issue, status
             FROM tasks WHERE project_id = ?1 ORDER BY name",
        )?;
        let rows = stmt
            .query_map(params![project_id], Self::row_to_task)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn find_session_by_task(&self, task_id: i64) -> Result<Option<Session>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, task_id, tmux_session_name, github_branch, worktree_path
                 FROM sessions WHERE task_id = ?1",
                params![task_id],
                Self::row_to_session,
            )
            .optional()?)
    }

    pub fn find_session_by_tmux_name(&self, tmux_name: &str) -> Result<Option<Session>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, task_id, tmux_session_name, github_branch, worktree_path
                 FROM sessions WHERE tmux_session_name = ?1",
                params![tmux_name],
                Self::row_to_session,
            )
            .optional()?)
    }

    pub fn records_for_task(&self, task_id: i64) -> Result<Vec<Record>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, task_id, start, end, message FROM records WHERE task_id = ?1")?;
        let rows = stmt
            .query_map(params![task_id], Self::row_to_record)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Every record across every task of `project_id` -- the union a
    /// project-level report is built from (see `crate::reporting`).
    pub fn records_for_project(&self, project_id: i64) -> Result<Vec<Record>> {
        let mut stmt = self.conn.prepare(
            "SELECT r.id, r.task_id, r.start, r.end, r.message
             FROM records r JOIN tasks t ON t.id = r.task_id
             WHERE t.project_id = ?1",
        )?;
        let rows = stmt
            .query_map(params![project_id], Self::row_to_record)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn open_record_for_task(&self, task_id: i64) -> Result<Option<Record>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, task_id, start, end, message
                 FROM records WHERE task_id = ?1 AND end IS NULL",
                params![task_id],
                Self::row_to_record,
            )
            .optional()?)
    }

    // ---- row mapping (signatures are pinned to `rusqlite::Result` by the
    // callback types `query_row`/`query_map` expect -- converted to our
    // `Result` at the call sites above instead) ---------------------------

    fn row_to_project(row: &Row) -> rusqlite::Result<Project> {
        Ok(Project {
            id: Some(row.get(0)?),
            name: row.get(1)?,
            description: row.get(2)?,
            base_path: row.get(3)?,
            github: row.get::<_, i64>(4)? != 0,
            tmux: row.get::<_, i64>(5)? != 0,
            auto_branch: row.get::<_, i64>(6)? != 0,
            branch_template: row.get(7)?,
        })
    }

    fn row_to_task(row: &Row) -> rusqlite::Result<Task> {
        let status: String = row.get(5)?;
        Ok(Task {
            id: Some(row.get(0)?),
            project_id: row.get(1)?,
            name: row.get(2)?,
            description: row.get(3)?,
            github_issue: row.get(4)?,
            status: TaskStatus::parse(&status).unwrap_or(TaskStatus::Queue),
        })
    }

    fn row_to_session(row: &Row) -> rusqlite::Result<Session> {
        Ok(Session {
            id: Some(row.get(0)?),
            task_id: row.get(1)?,
            tmux_session_name: row.get(2)?,
            github_branch: row.get(3)?,
            worktree_path: row.get(4)?,
        })
    }

    fn row_to_record(row: &Row) -> rusqlite::Result<Record> {
        let start: String = row.get(2)?;
        let end: Option<String> = row.get(3)?;
        Ok(Record {
            id: Some(row.get(0)?),
            task_id: row.get(1)?,
            start: str_to_dt(&start),
            end: end.as_deref().map(str_to_dt),
            message: row.get(4)?,
        })
    }
}

impl Repository<Project> for Db {
    fn insert(&self, item: &Project) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO projects (name, description, base_path, github, tmux, auto_branch, branch_template)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                item.name,
                item.description,
                item.base_path,
                item.github as i64,
                item.tmux as i64,
                item.auto_branch as i64,
                item.branch_template
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    fn update(&self, id: i64, item: &Project) -> Result<()> {
        self.conn.execute(
            "UPDATE projects SET name = ?1, description = ?2, base_path = ?3, github = ?4,
             tmux = ?5, auto_branch = ?6, branch_template = ?7 WHERE id = ?8",
            params![
                item.name,
                item.description,
                item.base_path,
                item.github as i64,
                item.tmux as i64,
                item.auto_branch as i64,
                item.branch_template,
                id
            ],
        )?;
        Ok(())
    }

    fn delete(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM projects WHERE id = ?1", params![id])?;
        Ok(())
    }

    fn get(&self, id: i64) -> Result<Option<Project>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, name, description, base_path, github, tmux, auto_branch, branch_template
                 FROM projects WHERE id = ?1",
                params![id],
                Self::row_to_project,
            )
            .optional()?)
    }

    fn list(&self) -> Result<Vec<Project>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, description, base_path, github, tmux, auto_branch, branch_template
             FROM projects ORDER BY name",
        )?;
        let rows = stmt
            .query_map([], Self::row_to_project)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
}

impl Repository<Task> for Db {
    fn insert(&self, item: &Task) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO tasks (project_id, name, description, github_issue, status)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                item.project_id,
                item.name,
                item.description,
                item.github_issue,
                item.status.as_str()
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    fn update(&self, id: i64, item: &Task) -> Result<()> {
        self.conn.execute(
            "UPDATE tasks SET project_id = ?1, name = ?2, description = ?3, github_issue = ?4,
             status = ?5 WHERE id = ?6",
            params![
                item.project_id,
                item.name,
                item.description,
                item.github_issue,
                item.status.as_str(),
                id
            ],
        )?;
        Ok(())
    }

    fn delete(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM tasks WHERE id = ?1", params![id])?;
        Ok(())
    }

    fn get(&self, id: i64) -> Result<Option<Task>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, project_id, name, description, github_issue, status FROM tasks WHERE id = ?1",
                params![id],
                Self::row_to_task,
            )
            .optional()?)
    }

    fn list(&self) -> Result<Vec<Task>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, project_id, name, description, github_issue, status FROM tasks ORDER BY name")?;
        let rows = stmt
            .query_map([], Self::row_to_task)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
}

impl Repository<Session> for Db {
    fn insert(&self, item: &Session) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO sessions (task_id, tmux_session_name, github_branch, worktree_path)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                item.task_id,
                item.tmux_session_name,
                item.github_branch,
                item.worktree_path
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    fn update(&self, id: i64, item: &Session) -> Result<()> {
        self.conn.execute(
            "UPDATE sessions SET task_id = ?1, tmux_session_name = ?2, github_branch = ?3,
             worktree_path = ?4 WHERE id = ?5",
            params![
                item.task_id,
                item.tmux_session_name,
                item.github_branch,
                item.worktree_path,
                id
            ],
        )?;
        Ok(())
    }

    fn delete(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM sessions WHERE id = ?1", params![id])?;
        Ok(())
    }

    fn get(&self, id: i64) -> Result<Option<Session>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, task_id, tmux_session_name, github_branch, worktree_path
                 FROM sessions WHERE id = ?1",
                params![id],
                Self::row_to_session,
            )
            .optional()?)
    }

    fn list(&self) -> Result<Vec<Session>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, task_id, tmux_session_name, github_branch, worktree_path FROM sessions",
        )?;
        let rows = stmt
            .query_map([], Self::row_to_session)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
}

impl Repository<Record> for Db {
    fn insert(&self, item: &Record) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO records (task_id, start, end, message) VALUES (?1, ?2, ?3, ?4)",
            params![
                item.task_id,
                dt_to_str(item.start),
                item.end.map(dt_to_str),
                item.message
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    fn update(&self, id: i64, item: &Record) -> Result<()> {
        self.conn.execute(
            "UPDATE records SET task_id = ?1, start = ?2, end = ?3, message = ?4 WHERE id = ?5",
            params![
                item.task_id,
                dt_to_str(item.start),
                item.end.map(dt_to_str),
                item.message,
                id
            ],
        )?;
        Ok(())
    }

    fn delete(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM records WHERE id = ?1", params![id])?;
        Ok(())
    }

    fn get(&self, id: i64) -> Result<Option<Record>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, task_id, start, end, message FROM records WHERE id = ?1",
                params![id],
                Self::row_to_record,
            )
            .optional()?)
    }

    fn list(&self) -> Result<Vec<Record>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, task_id, start, end, message FROM records")?;
        let rows = stmt
            .query_map([], Self::row_to_record)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
}
