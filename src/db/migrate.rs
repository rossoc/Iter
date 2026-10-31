//! Schema migrations: the versioned history and the steps it runs.

use super::*;

impl Db {
    /// Brings the schema up to date. `PRAGMA user_version` records how many
    /// of [`Self::MIGRATIONS`] have run, so an up-to-date database costs one
    /// pragma read to open -- which matters to `iter serve`, opening one per
    /// request, and to shell completion, opening one per TAB. Each step runs
    /// in its own transaction together with bumping the version, so a step
    /// either lands whole or runs again next time.
    ///
    /// Two processes can open an old database at once (a tmux hook and an
    /// `iter serve` request, say). Each step's transaction is therefore
    /// `IMMEDIATE` -- it takes the write lock up front, where `busy_timeout`
    /// waits for it, rather than upgrading a read lock mid-step, which fails
    /// outright -- and re-reads the version under that lock, so the process
    /// that waited skips a step the other one already ran.
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

    /// How many of [`Self::MIGRATIONS`] this database has run.
    pub(super) fn user_version(&self) -> Result<usize> {
        let done: i64 = self
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))?;
        Ok(usize::try_from(done).unwrap_or(0))
    }

    /// The schema's history, oldest first. Append a step; never edit one
    /// that has shipped.
    pub(super) const MIGRATIONS: &[fn(&Db) -> Result<()>] = &[
        Self::baseline,
        Self::flags_to_tags,
        Self::drop_short_sessions,
        Self::add_notes_path,
        Self::add_platform,
        Self::add_pushes,
    ];

    /// `notes_path` on projects and organizations: a database already past
    /// the baseline never runs [`Self::add_missing_columns`] again, so the
    /// columns added since need a step of their own.
    pub(super) fn add_notes_path(&self) -> Result<()> {
        self.add_missing_columns()
    }

    /// The time platform's connection settings on organizations.
    pub(super) fn add_platform(&self) -> Result<()> {
        self.add_missing_columns()
    }

    /// The ledger of worklogs pushed to a time platform.
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

    /// Everything from before versioning: idempotent, since a database
    /// that predates it could be at any point in that history.
    ///
    /// The `CREATE TABLE`s hold each table's original columns; every column
    /// added later is declared once, in [`Self::add_missing_columns`], which
    /// a fresh database runs too.
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

    /// Urgent/important moved from two `tasks` columns onto the two
    /// built-in tags (see [`Priority`]): each set flag becomes a tag row, and
    /// the column goes. A database that never had the columns has nothing
    /// to move.
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

    /// Closed sessions under [`Session::MIN_SECONDS`] were stored before
    /// closing learned to drop them; they go now. Measured in whole seconds
    /// (`strftime('%s')`): `julianday` differences carry float error that
    /// puts an exact minute at 59.99... Open sessions are left alone. A
    /// table without the two columns has no durations to measure.
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

    /// The two tags every database has: the ones the board views colour the
    /// Eisenhower flags with. `INSERT OR IGNORE` on the unique name, so a
    /// user's recolouring survives and re-running this is a no-op.
    pub(super) fn seed_tags(&self) -> Result<()> {
        for tag in Tag::defaults() {
            self.conn.execute(
                "INSERT OR IGNORE INTO tags (name, color, description) VALUES (?1, ?2, ?3)",
                params![tag.name, tag.color, tag.description],
            )?;
        }
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

    /// Asked the same way as [`Self::table_exists`] -- a single row that
    /// either comes back or doesn't, rather than every column name
    /// collected into a `Vec` to be scanned afterwards.
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

    /// One-time rename from the pre-rename schema (`Session`/`Record`
    /// structs, backed by tables `sessions`/`records`) to the current one
    /// (`SessionConfig`/`Session`, backed by tables `session_configs`/
    /// `sessions`). Each rename is guarded on its target name not already
    /// existing, so this is a no-op once applied, and a no-op on a fresh
    /// database that never had the old tables to begin with. Order matters:
    /// the old `sessions` table is renamed out of the way first, freeing
    /// that name for the old `records` table to take.
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
