use chrono::{Datelike, NaiveDate, NaiveDateTime, Weekday};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// One reconstructed begin -> end pair for a given name.
///
/// `end: None` means the session is still open (a trailing, unterminated
/// `begin` at the point the log was read) — see `is_ongoing`.
#[derive(Debug, PartialEq)]
pub struct SessionRecord {
    pub name: String,
    pub begin: NaiveDateTime,
    pub end: Option<NaiveDateTime>,
    pub message: Option<String>,
}

impl SessionRecord {
    /// Whether this session is still open, i.e. has no `end` recorded yet.
    pub fn is_ongoing(&self) -> bool {
        self.end.is_none()
    }

    /// Minutes spent in this session, measured against `end` when closed,
    /// or against `now` when still ongoing. Never negative.
    pub fn duration_minutes(&self, now: NaiveDateTime) -> i64 {
        let end = self.end.unwrap_or(now);
        (end - self.begin).num_minutes().max(0)
    }
}

/// The distinct session names present in `sessions`, sorted alphabetically.
pub fn distinct_names(sessions: &[SessionRecord]) -> Vec<String> {
    let mut names: Vec<String> = sessions
        .iter()
        .map(|s| s.name.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    names.sort();
    names
}

#[derive(Debug, Serialize)]
pub struct WeekdayAverage {
    pub weekday: String,
    pub average_hours: f64,
}

/// For each weekday Monday..Sunday (always all seven, zero-filled when
/// there's no data), the average hours logged on that weekday: total
/// minutes across `sessions` that fall on that weekday, divided by the
/// number of distinct calendar dates in `sessions` that fall on it. A
/// session is attributed to its `begin` date's weekday. Callers filter
/// `sessions` to one name beforehand to get a per-name average.
pub fn weekday_averages(sessions: &[SessionRecord], now: NaiveDateTime) -> Vec<WeekdayAverage> {
    let mut minutes_by_weekday: HashMap<Weekday, i64> = HashMap::new();
    let mut dates_by_weekday: HashMap<Weekday, HashSet<NaiveDate>> = HashMap::new();

    for s in sessions {
        let date = s.begin.date();
        let wd = date.weekday();
        *minutes_by_weekday.entry(wd).or_insert(0) += s.duration_minutes(now);
        dates_by_weekday.entry(wd).or_default().insert(date);
    }

    let order = [
        Weekday::Mon,
        Weekday::Tue,
        Weekday::Wed,
        Weekday::Thu,
        Weekday::Fri,
        Weekday::Sat,
        Weekday::Sun,
    ];

    order
        .iter()
        .map(|&wd| {
            let total_minutes = *minutes_by_weekday.get(&wd).unwrap_or(&0);
            let distinct_days = dates_by_weekday.get(&wd).map(|s| s.len()).unwrap_or(0);
            let average_hours = if distinct_days == 0 {
                0.0
            } else {
                (total_minutes as f64 / 60.0) / distinct_days as f64
            };
            WeekdayAverage {
                weekday: weekday_name(wd),
                average_hours,
            }
        })
        .collect()
}

fn weekday_name(wd: Weekday) -> String {
    match wd {
        Weekday::Mon => "Monday",
        Weekday::Tue => "Tuesday",
        Weekday::Wed => "Wednesday",
        Weekday::Thu => "Thursday",
        Weekday::Fri => "Friday",
        Weekday::Sat => "Saturday",
        Weekday::Sun => "Sunday",
    }
    .to_string()
}

pub fn minutes_to_hhmm(total_minutes: i64) -> String {
    format!("{:02}:{:02}", total_minutes / 60, total_minutes % 60)
}

pub fn round_to_half_hour(hours: f64) -> f64 {
    (hours * 2.0).round() / 2.0
}

/// Formats the messages recorded on `sessions`' closing `end` events as a
/// bulleted, copy-paste-ready list.
pub fn concat_messages(sessions: &[SessionRecord]) -> Option<String> {
    let messages: Vec<String> = sessions
        .iter()
        .filter_map(|s| s.message.as_deref())
        .map(|m| format!("- {}", m.trim_start_matches('-').trim_start()))
        .collect();
    if messages.is_empty() {
        None
    } else {
        Some(messages.join("\n"))
    }
}

/// Total minutes covered by `records`, merging any two chronologically
/// adjacent sessions (by begin time) whose gap is under `gap_minutes` into
/// one continuous span.
pub fn merged_total_minutes(
    records: &[SessionRecord],
    now: NaiveDateTime,
    gap_minutes: i64,
) -> i64 {
    let mut spans: Vec<(NaiveDateTime, NaiveDateTime)> = records
        .iter()
        .map(|r| (r.begin, r.end.unwrap_or(now)))
        .collect();
    spans.sort_by_key(|&(begin, _)| begin);

    let mut total = 0i64;
    let mut current: Option<(NaiveDateTime, NaiveDateTime)> = None;

    for (begin, end) in spans {
        current = match current {
            None => Some((begin, end)),
            Some((cur_begin, cur_end)) if (begin - cur_end).num_minutes() < gap_minutes => {
                Some((cur_begin, end.max(cur_end)))
            }
            Some((cur_begin, cur_end)) => {
                total += (cur_end - cur_begin).num_minutes();
                Some((begin, end))
            }
        };
    }
    if let Some((begin, end)) = current {
        total += (end - begin).num_minutes();
    }

    total.max(0)
}

// ---- YAML output shapes -----------------------------------------------

/// A day's worth of one session name: total time spent and every message
/// recorded on it, summed/concatenated across every begin/end pair that day.
#[derive(Debug, Serialize)]
pub struct SessionDetailReport {
    pub name: String,
    pub date: String,
    pub total_hours: f64,
    pub total_hhmm: String,
    #[serde(skip)]
    pub messages: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SessionWeekdayReport {
    pub name: String,
    pub weekdays: Vec<WeekdayAverage>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csv_log::parse_sessions;
    use chrono::NaiveDate;

    fn now() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 9, 4)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap()
    }

    #[test]
    fn is_ongoing_reflects_whether_end_is_set() {
        let begin = now();
        let open = SessionRecord {
            name: "A".to_string(),
            begin,
            end: None,
            message: None,
        };
        let closed = SessionRecord {
            name: "A".to_string(),
            begin,
            end: Some(begin),
            message: None,
        };
        assert!(open.is_ongoing());
        assert!(!closed.is_ongoing());
    }

    #[test]
    fn distinct_names_are_sorted_and_deduplicated() {
        let csv = "\
B,2026-09-01 09:00,2026-09-01 10:00,\n\
A,2026-09-01 11:00,2026-09-01 12:00,\n\
B,2026-09-02 09:00,2026-09-02 10:00,\n";
        let sessions = parse_sessions(csv);
        assert_eq!(distinct_names(&sessions), vec!["A", "B"]);
    }

    #[test]
    fn round_to_half_hour_below_quarter_rounds_down() {
        assert_eq!(round_to_half_hour(3.1), 3.0);
        assert_eq!(round_to_half_hour(3.0), 3.0);
    }

    #[test]
    fn round_to_half_hour_at_quarter_rounds_up_to_half() {
        assert_eq!(round_to_half_hour(3.25), 3.5);
    }

    #[test]
    fn round_to_half_hour_between_quarter_and_three_quarters_rounds_to_half() {
        assert_eq!(round_to_half_hour(3.4), 3.5);
        assert_eq!(round_to_half_hour(3.5), 3.5);
        assert_eq!(round_to_half_hour(3.6), 3.5);
    }

    #[test]
    fn round_to_half_hour_at_three_quarters_rounds_up_to_next_int() {
        assert_eq!(round_to_half_hour(3.75), 4.0);
    }

    #[test]
    fn round_to_half_hour_above_three_quarters_rounds_up_to_next_int() {
        assert_eq!(round_to_half_hour(3.9), 4.0);
    }

    #[test]
    fn concat_messages_joins_present_messages_and_skips_none() {
        let csv = "\
A,2026-09-01 09:00,2026-09-01 10:00,first\n\
A,2026-09-01 11:00,2026-09-01 12:00,\n\
A,2026-09-01 13:00,2026-09-01 14:00,second\n";
        let sessions = parse_sessions(csv);
        assert_eq!(
            concat_messages(&sessions),
            Some("- first\n- second".to_string())
        );
    }

    #[test]
    fn concat_messages_is_none_when_no_session_has_one() {
        let csv = "A,2026-09-01 09:00,2026-09-01 10:00,\n";
        let sessions = parse_sessions(csv);
        assert_eq!(concat_messages(&sessions), None);
    }

    #[test]
    fn concat_messages_does_not_double_an_existing_leading_dash() {
        let csv = "A,2026-09-01 09:00,2026-09-01 10:00,- already bulleted\n";
        let sessions = parse_sessions(csv);
        assert_eq!(
            concat_messages(&sessions),
            Some("- already bulleted".to_string())
        );
    }

    #[test]
    fn merged_total_bridges_gaps_under_threshold() {
        // 09:00-10:00, gap of 10 min, 10:10-11:00 -> merged span 09:00-11:00 = 120 min,
        // not the 60+50=110 min the two durations alone would sum to.
        let csv = "A,2026-09-01 09:00,2026-09-01 10:00,\nA,2026-09-01 10:10,2026-09-01 11:00,\n";
        let sessions = parse_sessions(csv);
        assert_eq!(merged_total_minutes(&sessions, now(), 17), 120);
    }

    #[test]
    fn merged_total_keeps_gaps_at_or_above_threshold_separate() {
        // Gap is exactly 17 min -> "closer than 17" is false, so the two
        // sessions are NOT merged; the gap itself isn't counted.
        let csv = "A,2026-09-01 09:00,2026-09-01 10:00,\nA,2026-09-01 10:17,2026-09-01 11:00,\n";
        let sessions = parse_sessions(csv);
        assert_eq!(merged_total_minutes(&sessions, now(), 17), 60 + 43);
    }

    #[test]
    fn merged_total_chains_across_more_than_two_sessions() {
        let csv = "\
A,2026-09-01 09:00,2026-09-01 09:30,\n\
A,2026-09-01 09:35,2026-09-01 10:00,\n\
A,2026-09-01 10:10,2026-09-01 10:40,\n";
        let sessions = parse_sessions(csv);
        // All gaps (5 min, 10 min) are under 17 -> one 09:00-10:40 span = 100 min.
        assert_eq!(merged_total_minutes(&sessions, now(), 17), 100);
    }

    #[test]
    fn merged_total_extends_into_an_ongoing_session() {
        let csv = "A,2026-09-04 09:00,2026-09-04 10:00,\nA,2026-09-04 10:05,,\n";
        let sessions = parse_sessions(csv);
        // now() is 2026-09-04 12:00 -> ongoing session runs 10:05-12:00, merged
        // with the prior 09:00-10:00 (5 min gap) into 09:00-12:00 = 180 min.
        assert_eq!(merged_total_minutes(&sessions, now(), 17), 180);
    }

    #[test]
    fn weekday_averages_basic() {
        // Monday 2026-08-31 and Monday 2026-09-07, Wednesday 2026-09-02.
        let csv = "\
A,2026-08-31 09:00,2026-08-31 11:00,\n\
A,2026-09-07 09:00,2026-09-07 13:00,\n\
A,2026-09-02 09:00,2026-09-02 12:00,\n";
        let sessions = parse_sessions(csv);
        let averages = weekday_averages(&sessions, now());
        assert_eq!(averages.len(), 7);
        assert_eq!(averages[0].weekday, "Monday");
        assert_eq!(averages[0].average_hours, 3.0); // (2 + 4) / 2 distinct Mondays
        assert_eq!(averages[2].weekday, "Wednesday");
        assert_eq!(averages[2].average_hours, 3.0); // 3 / 1
        assert_eq!(averages[1].average_hours, 0.0); // Tuesday, no data
    }

    #[test]
    fn weekday_averages_no_data_is_zero_filled() {
        let averages = weekday_averages(&[], now());
        assert_eq!(averages.len(), 7);
        assert!(averages.iter().all(|a| a.average_hours == 0.0));
        assert_eq!(averages[0].weekday, "Monday");
        assert_eq!(averages[6].weekday, "Sunday");
    }

    #[test]
    fn weekday_averages_respects_prefiltering_by_name() {
        let csv = "A,2026-08-31 09:00,2026-08-31 11:00,\nB,2026-08-31 09:00,2026-08-31 13:00,\n";
        let sessions = parse_sessions(csv);
        let a_only: Vec<_> = sessions.into_iter().filter(|s| s.name == "A").collect();
        let averages = weekday_averages(&a_only, now());
        assert_eq!(averages[0].average_hours, 2.0);
    }
}
