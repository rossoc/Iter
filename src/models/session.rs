use chrono::NaiveDateTime;

/// One stretch of work on a task: a start time, an optional end (`None`
/// means still open/ongoing), and an optional note. Started/stopped by
/// `iter session start`/`iter session stop` (manually) or the tmux
/// `client-attached`/`client-detached` hooks (automatically). Always keyed
/// by `task_id`, never by `SessionConfig` -- so a project's daily total is
/// just the union of every task's sessions that day, independent of how
/// many tmux sessions came and went (see
/// `crate::reporting::merged_total_minutes`).
#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    pub id: Option<i64>,
    pub task_id: i64,
    pub start: NaiveDateTime,
    pub end: Option<NaiveDateTime>,
    pub message: Option<String>,
}

impl Session {
    /// Whether this session is still open, i.e. has no `end` recorded yet.
    pub fn is_ongoing(&self) -> bool {
        self.end.is_none()
    }

    /// Minutes spent in this session, measured against `end` when closed,
    /// or against `now` when still ongoing. Never negative.
    pub fn duration_minutes(&self, now: NaiveDateTime) -> i64 {
        let end = self.end.unwrap_or(now);
        (end - self.start).num_minutes().max(0)
    }
}
