//! The session reads that reporting runs over a date range.

use super::*;

impl Db {
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

    /// Sessions in range of any task of `project_id`, oldest first; one query, each row carries `task_id`.
    pub fn sessions_for_project_in_range(
        &self,
        project_id: i64,
        from: Option<NaiveDate>,
        to: NaiveDate,
    ) -> Result<Vec<Session>> {
        self.sessions_for_projects_in_range(&[project_id], from, to)
    }

    /// The same for a set of projects.
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
}
