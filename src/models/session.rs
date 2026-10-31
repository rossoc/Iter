use crate::models::Duration;
use chrono::NaiveDateTime;
use iter_macros::Table;

/// One stretch of work on a task; `end` of `None` means still open. Keyed by `task_id`, never
/// `SessionConfig`, so a day's total is the union of a project's task sessions (see
/// `crate::reporting::merged_total_minutes`).
#[derive(Debug, Clone, PartialEq, Table)]
#[table(name = "sessions")]
pub struct Session {
    pub id: Option<i64>,
    pub task_id: i64,
    #[table(with = "datetime")]
    pub start: NaiveDateTime,
    #[table(with = "datetime::opt")]
    pub end: Option<NaiveDateTime>,
    pub message: Option<String>,
}

impl Session {
    /// The shortest stretch worth keeping; shorter is an attach/detach blip, never stored (see
    /// `utils::clock::close_open_session`).
    pub const MIN_SECONDS: i64 = 60;

    pub fn too_short(&self, end: NaiveDateTime) -> bool {
        (end - self.start).num_seconds() < Self::MIN_SECONDS
    }

    pub fn is_ongoing(&self) -> bool {
        self.end.is_none()
    }

    /// Minutes against `end`, or `now` when open. Never negative.
    pub fn duration_minutes(&self, now: NaiveDateTime) -> i64 {
        let end = self.end.unwrap_or(now);
        (end - self.start).num_minutes().max(0)
    }

    pub fn duration_text(&self, now: NaiveDateTime) -> String {
        Duration(self.duration_minutes(now)).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::dt as at;

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
        assert_eq!(s.duration_text(at("2025-03-03 08:00")), "00:00");
    }
}
