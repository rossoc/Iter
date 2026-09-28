use chrono::NaiveDateTime;
use iter_macros::Table;

/// One stretch of work on a task: a start time, an optional end (`None`
/// means still open/ongoing), and an optional note. Started/stopped by
/// `iter session start`/`iter session stop` (manually) or the tmux
/// `client-attached`/`client-detached` hooks (automatically). Always keyed
/// by `task_id`, never by `SessionConfig` -- so a project's daily total is
/// just the union of every task's sessions that day, independent of how
/// many tmux sessions came and went (see
/// `crate::reporting::merged_total_minutes`).
#[derive(Debug, Clone, PartialEq, Table)]
#[table(name = "sessions")]
pub struct Session {
    pub id: Option<i64>,
    pub task_id: i64,
    pub start: NaiveDateTime,
    pub end: Option<NaiveDateTime>,
    pub message: Option<String>,
}

impl Session {
    /// The shortest stretch of work worth keeping. Anything shorter is an
    /// attach/detach blip -- a quick look at a tmux session, a start undone
    /// by a stop -- not work, and is never stored (see
    /// `utils::clock::close_open_session`).
    pub const MIN_SECONDS: i64 = 60;

    /// Whether this session, ended at `end`, is too short to keep.
    pub fn too_short(&self, end: NaiveDateTime) -> bool {
        (end - self.start).num_seconds() < Self::MIN_SECONDS
    }

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
