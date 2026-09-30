//! The mapping from sessions to what the sessions table shows
//! (`ui::sessions_table`): the one place a session's times are written.

use super::ui::sessions_table::{Moment, SessionLine};
use crate::models::{START_TIME_FMT, Session, end_text};
use crate::reporting::{SessionRow, non_empty};
use chrono::NaiveDateTime;

/// The lines of an Organization report's task: the report's own strings.
pub fn report_lines(rows: &[SessionRow]) -> Vec<SessionLine<'_>> {
    rows.iter()
        .map(|r| SessionLine {
            date: r.date.as_deref().map(Into::into),
            start: Moment {
                text: r.start.as_str().into(),
                at: None,
            },
            end: r.end.as_deref().map(|text| Moment {
                text: text.into(),
                at: None,
            }),
            duration: r.duration.as_str().into(),
            note: r.message.as_deref().map(Into::into),
        })
        .collect()
}

/// The format of the HTML `datetime` attribute: `START_TIME_FMT` with the
/// `T` the attribute wants between the date and the time.
const DATETIME_ATTR: &str = "%Y-%m-%dT%H:%M";

/// A moment with its machine-readable form.
fn timed(t: NaiveDateTime, text: String) -> Moment<'static> {
    Moment {
        text: text.into(),
        at: Some(t.format(DATETIME_ATTR).to_string()),
    }
}

/// The lines of a task's sessions, in the order given. The start is dated;
/// an end on the same day is just the time. An open session is measured
/// against `now`.
pub fn task_lines(sessions: &[Session], now: NaiveDateTime) -> Vec<SessionLine<'_>> {
    sessions
        .iter()
        .map(|s| SessionLine {
            date: None,
            start: timed(s.start, s.start.format(START_TIME_FMT).to_string()),
            end: s.end.map(|end| timed(end, end_text(s.start, end))),
            duration: s.duration_text(now).into(),
            note: s.message.as_deref().and_then(non_empty).map(Into::into),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(text, START_TIME_FMT).unwrap()
    }

    fn session(start: &str, end: Option<&str>, message: Option<&str>) -> Session {
        Session {
            id: None,
            task_id: 1,
            start: at(start),
            end: end.map(at),
            message: message.map(String::from),
        }
    }

    #[test]
    fn an_end_on_the_start_day_is_only_the_time() {
        let sessions = [
            session("2025-03-03 09:00", Some("2025-03-03 10:30"), Some("done")),
            session("2025-03-03 23:00", Some("2025-03-04 00:30"), Some("")),
        ];
        let lines = task_lines(&sessions, at("2025-03-05 12:00"));
        assert_eq!(lines[0].start.text, "2025-03-03 09:00");
        assert_eq!(lines[0].start.at.as_deref(), Some("2025-03-03T09:00"));
        let end = lines[0].end.as_ref().unwrap();
        assert_eq!(
            (end.text.as_ref(), end.at.as_deref()),
            ("10:30", Some("2025-03-03T10:30"))
        );
        assert_eq!(lines[0].duration, "01:30");
        assert_eq!(lines[0].note.as_deref(), Some("done"));
        assert_eq!(lines[1].end.as_ref().unwrap().text, "2025-03-04 00:30");
        assert_eq!(lines[1].note, None);
    }

    #[test]
    fn an_open_session_has_no_end_and_counts_to_now() {
        let sessions = [session("2025-03-03 09:00", None, None)];
        let lines = task_lines(&sessions, at("2025-03-03 10:15"));
        assert!(lines[0].end.is_none());
        assert_eq!(lines[0].duration, "01:15");
    }
}
