use crate::models::Duration;
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

    /// [`Self::duration_minutes`] as `hh:mm`: the one way a session's length
    /// is written (reports, the task page).
    pub fn duration_text(&self, now: NaiveDateTime) -> String {
        Duration(self.duration_minutes(now)).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::START_TIME_FMT;

    fn at(text: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(text, START_TIME_FMT).unwrap()
    }

    #[test]
    fn duration_text_is_hhmm_and_open_sessions_count_to_now() {
        let mut s = Session {
            id: None,
            task_id: 1,
            start: at("2025-03-03 09:00"),
            end: Some(at("2025-03-03 10:30")),
            message: None,
        };
        assert_eq!(s.duration_text(at("2025-03-09 00:00")), "01:30");
        s.end = None;
        assert_eq!(s.duration_text(at("2025-03-03 09:05")), "00:05");
        // Never negative.
        assert_eq!(s.duration_text(at("2025-03-03 08:00")), "00:00");
    }
}
