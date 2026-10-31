//! Session and session-config queries.

use super::*;

impl Db {
    pub fn find_session_config_by_task(&self, task_id: i64) -> Result<Option<SessionConfig>> {
        self.find_one("WHERE task_id = ?1", params![task_id])
    }

    /// Every session-config belonging to a task of `project_id` -- one
    /// query, rather than one per task.
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
}
