//! Session, session-config and push-ledger queries, with the date-range session reads that
//! reporting runs.

use super::*;

impl Db {
    pub fn find_session_config_by_task(&self, task_id: i64) -> Result<Option<SessionConfig>> {
        self.find_one("WHERE task_id = ?1", params![task_id])
    }

    /// Every session-config of a task of `project_id`.
    pub fn session_configs_for_project(&self, project_id: i64) -> Result<Vec<SessionConfig>> {
        self.find_all(
            "WHERE task_id IN (SELECT id FROM tasks WHERE project_id = ?1)",
            params![project_id],
        )
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

    pub fn open_session_for_task(&self, task_id: i64) -> Result<Option<Session>> {
        self.find_one("WHERE task_id = ?1 AND end IS NULL", params![task_id])
    }

    /// Sessions of `task_id` that *start* within the range, oldest first.
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

    /// Sessions in range of any task of the projects, oldest first; one query, each row carries `task_id`.
    pub fn sessions_for_projects_in_range(
        &self,
        project_ids: &[i64],
        from: Option<NaiveDate>,
        to: NaiveDate,
    ) -> Result<Vec<Session>> {
        let (lower, upper) = range_bounds(from, to);
        // Written out rather than built by `select_sql`: the join puts an
        // `id` column on both sides, so the select list has to qualify it.
        self.query_all(
            "SELECT s.id, s.task_id, s.start, s.end, s.message
             FROM sessions s JOIN tasks t ON t.id = s.task_id
             WHERE t.project_id IN (SELECT value FROM json_each(?1))
               AND s.start >= ?2 AND s.start < ?3
             ORDER BY s.start, s.id",
            params![json_ids(project_ids), lower, upper],
            Session::from_row,
        )
    }

    /// The push already recorded for `organization_id` with this fingerprint.
    pub fn find_push(&self, organization_id: i64, hash: &str) -> Result<Option<Push>> {
        self.find_one(
            "WHERE organization_id = ?1 AND hash = ?2 ORDER BY id DESC",
            params![organization_id, hash],
        )
    }
}
