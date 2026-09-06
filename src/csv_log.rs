use crate::session_log::SessionLog;
use crate::sessions::SessionRecord;
use chrono::NaiveDateTime;
use std::io::{Read, Write};
use std::path::PathBuf;

pub struct CSVLog {
    file: PathBuf,
}

impl CSVLog {
    pub fn new(file: &str) -> CSVLog {
        let file = PathBuf::from(file);

        if !file.exists() {
            std::fs::File::create(&file).unwrap();
        }

        CSVLog { file }
    }

    fn read_raw(&self) -> String {
        let mut file = std::fs::File::open(&self.file).unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        contents
    }

    fn append(&self, line: &str) {
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&self.file)
            .unwrap();
        writeln!(file, "{}", line).unwrap();
    }
}

impl SessionLog for CSVLog {
    fn insert(&self, record: &SessionRecord) {
        self.append(&format_row(record));
    }

    fn update(&self, record: &SessionRecord) {
        // One row per session, so "editing the last session" means rewriting
        // that row in place rather than appending a second one for it.
        let mut lines: Vec<String> = self.read_raw().lines().map(str::to_string).collect();

        // The row `end_with_message` meant to close: the last line whose
        // parsed session is still open under this name.
        let target = lines.iter().enumerate().rev().find_map(|(i, line)| {
            parse_session_row(line.trim())
                .filter(|s| s.is_ongoing() && s.name == record.name)
                .map(|_| i)
        });

        match target {
            Some(i) => lines[i] = format_row(record),
            // No open row found — append rather than lose the close.
            None => lines.push(format_row(record)),
        }

        let mut out = lines.join("\n");
        if !out.is_empty() {
            out.push('\n');
        }
        std::fs::write(&self.file, out).unwrap();
    }

    fn select(&self) -> Vec<SessionRecord> {
        parse_sessions(&self.read_raw())
    }
}

/// Formats one session as a CSV row: `name,begin,end,message`, with `end`
/// and `message` left blank while the session has neither.
fn format_row(record: &SessionRecord) -> String {
    let end = record
        .end
        .map(|e| e.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_default();
    let message = record
        .message
        .as_deref()
        .map(sanitize_message)
        .unwrap_or_default();
    format!(
        "{},{},{},{}",
        record.name,
        record.begin.format("%Y-%m-%d %H:%M"),
        end,
        message
    )
}

/// Replaces newlines with spaces so a message can never split into extra
/// log lines (the reader relies on one row per line). This is purely a
/// consequence of storing rows as CSV text — a real database column has no
/// such constraint — so it lives on `CSVLog` rather than on `SessionLog`.
fn sanitize_message(message: &str) -> String {
    message.replace(['\n', '\r'], " ")
}

/// Parses every row directly into a `SessionRecord` — the CSV columns
/// (`name`, `begin`, `end`, `message`) already are the columns a real
/// database table would have, so unlike the old event-log format there is no
/// begin/end reconciliation to do: each line already is one session, with
/// `end` and `message` blank while it's still open. This is CSV-text-specific
/// parsing, so it lives here rather than as a generic `SessionLog` concern —
/// another backend (e.g. a real database) would just query rows and would
/// have no use for it.
pub(crate) fn parse_sessions(csv: &str) -> Vec<SessionRecord> {
    let mut sessions: Vec<SessionRecord> = csv
        .lines()
        .filter_map(|line| parse_session_row(line.trim()))
        .collect();
    sessions.sort_by_key(|s| s.begin);
    sessions
}

/// Parses one `name,begin,end,message` row. A row that doesn't parse (wrong
/// column count, a bad timestamp) yields `None` rather than panicking, since
/// the log keeps growing by hand.
fn parse_session_row(line: &str) -> Option<SessionRecord> {
    if line.is_empty() {
        return None;
    }

    // splitn(4, ...) rather than a plain split, so a message containing a
    // comma stays intact as the last field instead of being cut apart.
    let parts: Vec<&str> = line.splitn(4, ',').collect();
    if parts.len() != 4 {
        return None;
    }

    let name = parts[0].trim();
    if name.is_empty() {
        return None;
    }

    let begin = NaiveDateTime::parse_from_str(parts[1].trim(), "%Y-%m-%d %H:%M").ok()?;

    let end_str = parts[2].trim();
    let end = if end_str.is_empty() {
        None
    } else {
        Some(NaiveDateTime::parse_from_str(end_str, "%Y-%m-%d %H:%M").ok()?)
    };

    let message = parts[3].trim();
    let message = if message.is_empty() {
        None
    } else {
        Some(message.to_string())
    };

    Some(SessionRecord {
        name: name.to_string(),
        begin,
        end,
        message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn temp_path(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "iter_csv_log_test_{}_{}.csv",
            name,
            std::process::id()
        ));
        p
    }

    fn now() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 9, 4)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap()
    }

    #[test]
    fn insert_then_select_round_trips_an_ongoing_session() {
        let path = temp_path("insert_select");
        let log = CSVLog::new(path.to_str().unwrap());
        let begin = NaiveDate::from_ymd_opt(2026, 9, 4)
            .unwrap()
            .and_hms_opt(9, 0, 0)
            .unwrap();

        log.insert(&SessionRecord {
            name: "A".to_string(),
            begin,
            end: None,
            message: None,
        });

        let sessions = log.select();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].name, "A");
        assert!(sessions[0].is_ongoing());

        std::fs::remove_file(path).ok();
    }

    #[test]
    fn update_closes_the_open_session_and_sanitizes_the_message() {
        let path = temp_path("update_sanitize");
        let log = CSVLog::new(path.to_str().unwrap());
        let begin = NaiveDate::from_ymd_opt(2026, 9, 4)
            .unwrap()
            .and_hms_opt(9, 0, 0)
            .unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 9, 4)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap();

        log.insert(&SessionRecord {
            name: "A".to_string(),
            begin,
            end: None,
            message: None,
        });
        log.update(&SessionRecord {
            name: "A".to_string(),
            begin,
            end: Some(end),
            message: Some("multi\nline".to_string()),
        });

        // One row per session: closing it edits that same row rather than
        // appending a second one, so exactly one session comes back.
        let sessions = log.select();
        assert_eq!(sessions.len(), 1);
        assert!(!sessions[0].is_ongoing());
        assert_eq!(sessions[0].message, Some("multi line".to_string()));

        std::fs::remove_file(path).ok();
    }

    #[test]
    fn update_appends_when_no_open_session_matches() {
        let path = temp_path("update_fallback");
        let log = CSVLog::new(path.to_str().unwrap());
        let begin = NaiveDate::from_ymd_opt(2026, 9, 4)
            .unwrap()
            .and_hms_opt(9, 0, 0)
            .unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 9, 4)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap();

        log.update(&SessionRecord {
            name: "A".to_string(),
            begin,
            end: Some(end),
            message: None,
        });

        let sessions = log.select();
        assert_eq!(sessions.len(), 1);
        assert!(!sessions[0].is_ongoing());

        std::fs::remove_file(path).ok();
    }

    #[test]
    fn sanitize_message_replaces_newlines_with_spaces() {
        assert_eq!(sanitize_message("a\nb\r\nc"), "a b  c");
    }

    // parse_sessions' own row-parsing behavior.

    #[test]
    fn parses_a_closed_session_row() {
        let csv = "A,2026-09-01 09:00,2026-09-01 10:30,\n";
        let sessions = parse_sessions(csv);
        assert_eq!(sessions.len(), 1);
        let s = &sessions[0];
        assert_eq!(s.name, "A");
        assert!(!s.is_ongoing());
        assert_eq!(s.duration_minutes(now()), 90);
        assert_eq!(s.message, None);
    }

    #[test]
    fn parses_an_open_session_row_as_ongoing() {
        let csv = "A,2026-09-04 10:00,,\n";
        let sessions = parse_sessions(csv);
        assert_eq!(sessions.len(), 1);
        let s = &sessions[0];
        assert!(s.is_ongoing());
        assert_eq!(s.end, None);
        assert_eq!(s.message, None);
        assert_eq!(s.duration_minutes(now()), 120);
    }

    #[test]
    fn captures_a_message_with_an_embedded_comma() {
        let csv = "A,2026-09-01 09:00,2026-09-01 10:00,wrapped up, with a comma\n";
        let sessions = parse_sessions(csv);
        assert_eq!(sessions.len(), 1);
        assert_eq!(
            sessions[0].message,
            Some("wrapped up, with a comma".to_string())
        );
    }

    #[test]
    fn blank_message_field_is_none() {
        let csv = "A,2026-09-01 09:00,2026-09-01 10:00,\n";
        let sessions = parse_sessions(csv);
        assert_eq!(sessions[0].message, None);
    }

    #[test]
    fn row_with_wrong_column_count_is_skipped() {
        let csv = "A,2026-09-01 09:00,2026-09-01 10:00\n"; // only 3 columns
        assert_eq!(parse_sessions(csv), vec![]);
    }

    #[test]
    fn row_with_a_bad_timestamp_is_skipped_without_panicking() {
        let csv = "A,not-a-timestamp,,\nA,2026-09-01 09:00,2026-09-01 10:00,\n";
        let sessions = parse_sessions(csv);
        assert_eq!(sessions.len(), 1);
    }

    #[test]
    fn two_names_parse_independently() {
        let csv = "A,2026-09-01 09:00,2026-09-01 10:00,\nB,2026-09-01 09:05,2026-09-01 10:10,\n";
        let sessions = parse_sessions(csv);
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].name, "A");
        assert_eq!(sessions[0].duration_minutes(now()), 60);
        assert_eq!(sessions[1].name, "B");
        assert_eq!(sessions[1].duration_minutes(now()), 65);
    }

    #[test]
    fn rows_out_of_order_are_sorted_by_begin() {
        let csv = "B,2026-09-01 09:05,2026-09-01 10:10,\nA,2026-09-01 09:00,2026-09-01 10:00,\n";
        let sessions = parse_sessions(csv);
        assert_eq!(sessions[0].name, "A");
        assert_eq!(sessions[1].name, "B");
    }
}
