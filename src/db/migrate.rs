//! Schema migrations: the versioned history and the steps it runs.

use super::*;

impl Db {
    /// Brings the schema up to date. `PRAGMA user_version` counts the [`Self::MIGRATIONS`] run, so an
    /// up-to-date open costs one pragma read (serve and completion open per request/TAB). Each step
    /// and its version bump share one `IMMEDIATE` transaction: the write lock is taken up front
    /// (where `busy_timeout` waits; a mid-step upgrade fails outright) and the version re-read
    /// under it, so a concurrent process skips a step already run.
    pub(super) fn migrate(&self) -> Result<()> {
        if self.user_version()? >= Self::MIGRATIONS.len() {
            return Ok(());
        }
        loop {
            let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
            let done = self.user_version()?;
            let Some(step) = Self::MIGRATIONS.get(done) else {
                return Ok(());
            };
            step(self)?;
            tx.execute_batch(&format!("PRAGMA user_version = {}", done + 1))?;
            tx.commit()?;
        }
    }

    pub(super) fn user_version(&self) -> Result<usize> {
        let done: i64 = self
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))?;
        Ok(usize::try_from(done).unwrap_or(0))
    }

    /// Oldest first. Append a step; never edit a shipped one.
    pub(super) const MIGRATIONS: &[fn(&Db) -> Result<()>] = &[
        Self::baseline,
        Self::flags_to_tags,
        Self::drop_short_sessions,
        Self::add_notes_path,
        Self::add_platform,
        Self::add_pushes,
    ];

    /// `notes_path` on projects and organizations.
    pub(super) fn add_notes_path(&self) -> Result<()> {
        self.add_missing_columns()
    }

    pub(super) fn add_platform(&self) -> Result<()> {
        self.add_missing_columns()
    }

    pub(super) fn add_pushes(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS pushes (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                organization_id INTEGER NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
                from_day        TEXT NOT NULL,
                to_day          TEXT NOT NULL,
                hash            TEXT NOT NULL,
                at              TEXT NOT NULL,
                results         TEXT NOT NULL DEFAULT ''
             );
             CREATE INDEX IF NOT EXISTS idx_pushes_organization ON pushes(organization_id, hash);",
        )?;
        Ok(())
    }

    /// Everything before versioning; idempotent. The `CREATE TABLE`s hold the original columns;
    /// later ones are declared once in [`Self::add_missing_columns`].
    pub(super) fn baseline(&self) -> Result<()> {
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

    /// Urgent/important move from two `tasks` columns onto the two built-in tags (see
    /// [`Priority`]); nothing to move if the columns never existed.
    pub(super) fn flags_to_tags(&self) -> Result<()> {
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

    /// Drops closed sessions under [`Session::MIN_SECONDS`]. Whole seconds (`strftime('%s')`):
    /// `julianday` float error puts an exact minute at 59.99... Open sessions stay.
    pub(super) fn drop_short_sessions(&self) -> Result<()> {
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

    /// The two seeded tags; `INSERT OR IGNORE` keeps a user's recolouring.
    pub(super) fn seed_tags(&self) -> Result<()> {
        for tag in Tag::defaults() {
            self.conn.execute(
                "INSERT OR IGNORE INTO tags (name, color, description) VALUES (?1, ?2, ?3)",
                params![tag.name, tag.color, tag.description],
            )?;
        }
        Ok(())
    }

    /// Indexes for lookups that would scan whole tables (`sessions` by `task_id` is walked once
    /// per task in a report; `start` rides along for range and order). Failures are swallowed: an
    /// index is only an optimisation and may name columns an old database lacks yet; it appears on
    /// the first run the schema can take it.
    pub(super) fn create_indexes(&self) {
        const INDEXES: [&str; 5] = [
            "CREATE INDEX IF NOT EXISTS idx_sessions_task_start
                ON sessions(task_id, start);",
            "CREATE INDEX IF NOT EXISTS idx_projects_organization
                ON projects(organization_id);",
            "CREATE INDEX IF NOT EXISTS idx_projects_board
                ON projects(board_id);",
            // Every tmux hook resolves its session this way.
            "CREATE INDEX IF NOT EXISTS idx_session_configs_tmux
                ON session_configs(tmux_session_name);",
            // `task_tags`' primary key leads with `task_id`, so it can't
            // answer "which tasks carry tag X" -- the priority lookup.
            "CREATE INDEX IF NOT EXISTS idx_task_tags_tag
                ON task_tags(tag_id);",
        ];
        for sql in INDEXES {
            let _ = self.conn.execute_batch(sql);
        }
    }

    /// Columns added after a table's creation, each guarded on being absent (`CREATE TABLE IF NOT
    /// EXISTS` won't add them). `organization_id` and `board_id` are `ON DELETE SET NULL`, not
    /// `CASCADE`: groups are containers, so deleting one must not take its projects, tasks and
    /// sessions with it.
    pub(super) fn add_missing_columns(&self) -> Result<()> {
        const ADDED: [(&str, &str, &str); 15] = [
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
            ("projects", "notes_path", "TEXT NOT NULL DEFAULT ''"),
            ("organizations", "notes_path", "TEXT NOT NULL DEFAULT ''"),
            ("organizations", "platform_url", "TEXT NOT NULL DEFAULT ''"),
            (
                "organizations",
                "platform_project",
                "TEXT NOT NULL DEFAULT ''",
            ),
            (
                "organizations",
                "platform_activity",
                "TEXT NOT NULL DEFAULT ''",
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

    pub(super) fn column_exists(&self, table: &str, column: &str) -> Result<bool> {
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

    /// One-time rename `sessions` -> `session_configs`, `records` -> `sessions`, each guarded on its
    /// target not existing. Order matters: the old `sessions` moves aside first, freeing the name.
    pub(super) fn rename_legacy_tables(&self) -> Result<()> {
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

    pub(super) fn table_exists(&self, name: &str) -> Result<bool> {
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
}
