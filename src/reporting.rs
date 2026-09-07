use crate::models::Session;
use chrono::{Datelike, NaiveDate, NaiveDateTime, Weekday};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// A gap between one record's end and the next record's start shorter than
/// this many minutes is treated as a pause within one continuous span (e.g.
/// a quick interruption) rather than a real break between records.
pub const MERGE_GAP_MINUTES: i64 = 17;

#[derive(Debug, Serialize)]
pub struct WeekdayAverage {
    pub weekday: &'static str,
    pub average_hours: f64,
}

/// The seven weekdays in report order (Monday first), each with the label
/// it's reported under -- one table, so the ordering and the names can't
/// drift apart the way a separate list and `match` could.
const WEEKDAYS: [(Weekday, &str); 7] = [
    (Weekday::Mon, "Monday"),
    (Weekday::Tue, "Tuesday"),
    (Weekday::Wed, "Wednesday"),
    (Weekday::Thu, "Thursday"),
    (Weekday::Fri, "Friday"),
    (Weekday::Sat, "Saturday"),
    (Weekday::Sun, "Sunday"),
];

/// For each weekday Monday..Sunday (always all seven, zero-filled when
/// there's no data), the average hours logged on that weekday: total
/// minutes across `sessions` that fall on that weekday, divided by the
/// number of distinct calendar dates in `sessions` that fall on it. A
/// session is attributed to its `start` date's weekday. Callers filter
/// `sessions` to one task (or one project) beforehand.
pub fn weekday_averages(sessions: &[Session], now: NaiveDateTime) -> Vec<WeekdayAverage> {
    let mut minutes_by_weekday: HashMap<Weekday, i64> = HashMap::new();
    let mut dates_by_weekday: HashMap<Weekday, HashSet<NaiveDate>> = HashMap::new();

    for r in sessions {
        let date = r.start.date();
        let wd = date.weekday();
        *minutes_by_weekday.entry(wd).or_insert(0) += r.duration_minutes(now);
        dates_by_weekday.entry(wd).or_default().insert(date);
    }

    WEEKDAYS
        .iter()
        .map(|&(wd, weekday)| {
            let total_minutes = *minutes_by_weekday.get(&wd).unwrap_or(&0);
            let distinct_days = dates_by_weekday.get(&wd).map(|s| s.len()).unwrap_or(0);
            let average_hours = if distinct_days == 0 {
                0.0
            } else {
                (total_minutes as f64 / 60.0) / distinct_days as f64
            };
            WeekdayAverage {
                weekday,
                average_hours,
            }
        })
        .collect()
}

/// The subset of `sessions` that started on `date` -- the day filter every
/// `info` report applies before summing.
pub fn on_date(sessions: Vec<Session>, date: NaiveDate) -> Vec<Session> {
    sessions
        .into_iter()
        .filter(|s| s.start.date() == date)
        .collect()
}

pub fn minutes_to_hhmm(total_minutes: i64) -> String {
    format!("{:02}:{:02}", total_minutes / 60, total_minutes % 60)
}

pub fn round_to_half_hour(hours: f64) -> f64 {
    (hours * 2.0).round() / 2.0
}

/// Formats the messages recorded on `sessions`' closing `end` events as a
/// bulleted, copy-paste-ready list.
pub fn concat_messages(sessions: &[Session]) -> Option<String> {
    let messages: Vec<String> = sessions
        .iter()
        .filter_map(|r| r.message.as_deref())
        .map(|m| format!("- {}", m.trim_start_matches('-').trim_start()))
        .collect();
    if messages.is_empty() {
        None
    } else {
        Some(messages.join("\n"))
    }
}

/// Total minutes covered by `sessions`, merging any two chronologically
/// adjacent sessions (by start time) whose gap is under `gap_minutes` into
/// one continuous span. This is what makes a project-level report correct
/// even when two of its tasks were worked on in parallel: feed it the union
/// of every task's sessions for the day and overlapping/near spans collapse
/// into one, instead of being double-counted.
pub fn merged_total_minutes(sessions: &[Session], now: NaiveDateTime, gap_minutes: i64) -> i64 {
    let mut spans: Vec<(NaiveDateTime, NaiveDateTime)> = sessions
        .iter()
        .map(|r| (r.start, r.end.unwrap_or(now)))
        .collect();
    spans.sort_by_key(|&(start, _)| start);

    let mut total = 0i64;
    let mut current: Option<(NaiveDateTime, NaiveDateTime)> = None;

    for (start, end) in spans {
        current = match current {
            None => Some((start, end)),
            Some((cur_start, cur_end)) if (start - cur_end).num_minutes() < gap_minutes => {
                Some((cur_start, end.max(cur_end)))
            }
            Some((cur_start, cur_end)) => {
                total += (cur_end - cur_start).num_minutes();
                Some((start, end))
            }
        };
    }
    if let Some((start, end)) = current {
        total += (end - start).num_minutes();
    }

    total.max(0)
}

// ---- YAML output shapes -----------------------------------------------

/// A day's worth of sessions (for one task, or unioned across one project):
/// total time spent and every message recorded, summed/concatenated across
/// every start/end pair that day.
#[derive(Debug, Serialize)]
pub struct DetailReport {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub date: String,
    pub total_hours: f64,
    pub total_hhmm: String,
    #[serde(skip)]
    pub messages: Option<String>,
    /// `Some` only for a project-level report: one entry per task that had
    /// a session on `date`. Its presence is what tells
    /// `format_detail_report` to render the per-task `tasks:` breakdown
    /// instead of the flat `messages:` field -- a task-level report always
    /// passes `None` and keeps the flat field.
    #[serde(skip)]
    pub tasks: Option<Vec<TaskEntry>>,
}

/// One task's line in a project report's `tasks:` breakdown: its name,
/// status, and that day's messages (already bulleted by `concat_messages`).
#[derive(Debug)]
pub struct TaskEntry {
    pub name: String,
    pub status: String,
    pub messages: Option<String>,
}

/// Renders `report` as YAML, appending `messages` (or, for a project
/// report, `tasks`) by hand right after serde_yaml's output instead of
/// letting it serialize the bulleted text as a normal string. A plain
/// scalar gets wrapped in single quotes (and any `'` inside doubled to
/// `''`) the moment it starts with `-` or contains a `'` — exactly what a
/// typed note tends to do — which breaks copy-pasting a message straight
/// back out of the terminal.
pub fn format_detail_report(report: &DetailReport) -> String {
    let mut out = serde_yaml::to_string(report).expect("yaml serialization failed");
    match &report.tasks {
        None => match &report.messages {
            Some(m) => {
                out.push_str("messages:\n");
                out.push_str(m);
                out.push('\n');
            }
            None => out.push_str("messages: null\n"),
        },
        Some(tasks) if tasks.is_empty() => out.push_str("tasks: []\n"),
        Some(tasks) => {
            out.push_str("tasks:\n");
            for t in tasks {
                out.push_str(&format!("- {}: {}\n", t.name, t.status));
                if let Some(m) = &t.messages {
                    out.push_str("  messages:\n");
                    for line in m.lines() {
                        out.push_str("  ");
                        out.push_str(line);
                        out.push('\n');
                    }
                }
            }
        }
    }
    out
}

/// One task's line in an organization report: just its name and status.
#[derive(Debug)]
pub struct TaskSummary {
    pub name: String,
    pub status: String,
}

/// One project's block in an organization report: the project's name and
/// every one of its tasks.
#[derive(Debug)]
pub struct ProjectSummary {
    pub name: String,
    pub tasks: Vec<TaskSummary>,
}

/// An organization at a glance: every project in it, and the status of each
/// of those projects' tasks. Deliberately carries no times and no messages
/// -- it's a roster of what exists and where it stands, not a time sheet;
/// `project info` and `task info` remain the place for a day's hours.
#[derive(Debug, Serialize)]
pub struct OrganizationReport {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Rendered by hand after serde_yaml's output, like `DetailReport`'s
    /// `tasks`: the nested project/task tree isn't a YAML shape.
    #[serde(skip)]
    pub projects: Vec<ProjectSummary>,
}

/// Renders `report` as its header followed by the project/task tree:
///
/// ```text
/// projects:
/// - a-project
///     - a-task: wip
/// ```
///
/// The header goes through serde_yaml (so a multi-line description is
/// quoted correctly); the tree is written by hand, since its indentation
/// isn't valid YAML nesting.
pub fn format_organization_report(report: &OrganizationReport) -> String {
    let mut out = serde_yaml::to_string(report).expect("yaml serialization failed");
    if report.projects.is_empty() {
        out.push_str("projects: []\n");
        return out;
    }
    out.push_str("projects:\n");
    for project in &report.projects {
        out.push_str(&format!("- {}\n", project.name));
        for task in &project.tasks {
            out.push_str(&format!("    - {}: {}\n", task.name, task.status));
        }
    }
    out
}

#[derive(Debug, Serialize)]
pub struct WeekdayReport {
    pub name: String,
    pub weekdays: Vec<WeekdayAverage>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn now() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 9, 4)
            .expect("2026-09-04 is a valid calendar date")
            .and_hms_opt(12, 0, 0)
            .expect("12:00:00 is a valid time")
    }

    fn dt(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M")
            .unwrap_or_else(|e| panic!("bad test fixture datetime '{s}': {e}"))
    }

    fn sess(task_id: i64, start: &str, end: Option<&str>, message: Option<&str>) -> Session {
        Session {
            id: None,
            task_id,
            start: dt(start),
            end: end.map(dt),
            message: message.map(|m| m.to_string()),
        }
    }

    #[test]
    fn is_ongoing_reflects_whether_end_is_set() {
        let open = sess(1, "2026-09-04 09:00", None, None);
        let closed = sess(1, "2026-09-04 09:00", Some("2026-09-04 09:00"), None);
        assert!(open.is_ongoing());
        assert!(!closed.is_ongoing());
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
        let sessions = vec![
            sess(
                1,
                "2026-09-01 09:00",
                Some("2026-09-01 10:00"),
                Some("first"),
            ),
            sess(1, "2026-09-01 11:00", Some("2026-09-01 12:00"), None),
            sess(
                1,
                "2026-09-01 13:00",
                Some("2026-09-01 14:00"),
                Some("second"),
            ),
        ];
        assert_eq!(
            concat_messages(&sessions),
            Some("- first\n- second".to_string())
        );
    }

    #[test]
    fn concat_messages_is_none_when_no_session_has_one() {
        let sessions = vec![sess(1, "2026-09-01 09:00", Some("2026-09-01 10:00"), None)];
        assert_eq!(concat_messages(&sessions), None);
    }

    #[test]
    fn concat_messages_does_not_double_an_existing_leading_dash() {
        let sessions = vec![sess(
            1,
            "2026-09-01 09:00",
            Some("2026-09-01 10:00"),
            Some("- already bulleted"),
        )];
        assert_eq!(
            concat_messages(&sessions),
            Some("- already bulleted".to_string())
        );
    }

    #[test]
    fn merged_total_bridges_gaps_under_threshold() {
        // 09:00-10:00, gap of 10 min, 10:10-11:00 -> merged span 09:00-11:00 = 120 min,
        // not the 60+50=110 min the two durations alone would sum to.
        let sessions = vec![
            sess(1, "2026-09-01 09:00", Some("2026-09-01 10:00"), None),
            sess(1, "2026-09-01 10:10", Some("2026-09-01 11:00"), None),
        ];
        assert_eq!(merged_total_minutes(&sessions, now(), 17), 120);
    }

    #[test]
    fn merged_total_keeps_gaps_at_or_above_threshold_separate() {
        // Gap is exactly 17 min -> "closer than 17" is false, so the two
        // sessions are NOT merged; the gap itself isn't counted.
        let sessions = vec![
            sess(1, "2026-09-01 09:00", Some("2026-09-01 10:00"), None),
            sess(1, "2026-09-01 10:17", Some("2026-09-01 11:00"), None),
        ];
        assert_eq!(merged_total_minutes(&sessions, now(), 17), 60 + 43);
    }

    #[test]
    fn merged_total_chains_across_more_than_two_sessions() {
        let sessions = vec![
            sess(1, "2026-09-01 09:00", Some("2026-09-01 09:30"), None),
            sess(1, "2026-09-01 09:35", Some("2026-09-01 10:00"), None),
            sess(1, "2026-09-01 10:10", Some("2026-09-01 10:40"), None),
        ];
        // All gaps (5 min, 10 min) are under 17 -> one 09:00-10:40 span = 100 min.
        assert_eq!(merged_total_minutes(&sessions, now(), 17), 100);
    }

    #[test]
    fn merged_total_extends_into_an_ongoing_session() {
        let sessions = vec![
            sess(1, "2026-09-04 09:00", Some("2026-09-04 10:00"), None),
            sess(1, "2026-09-04 10:05", None, None),
        ];
        // now() is 2026-09-04 12:00 -> ongoing session runs 10:05-12:00, merged
        // with the prior 09:00-10:00 (5 min gap) into 09:00-12:00 = 180 min.
        assert_eq!(merged_total_minutes(&sessions, now(), 17), 180);
    }

    #[test]
    fn merged_total_unions_across_different_tasks_without_double_counting() {
        // The scenario that motivated keying sessions by task: two different
        // tasks (of the same project) worked in parallel, overlapping
        // 09:00-10:00 and 09:30-10:30 -> union is 09:00-10:30 = 90 min, not
        // the 120 min the two durations would sum to if counted separately.
        let sessions = vec![
            sess(1, "2026-09-01 09:00", Some("2026-09-01 10:00"), None),
            sess(2, "2026-09-01 09:30", Some("2026-09-01 10:30"), None),
        ];
        assert_eq!(merged_total_minutes(&sessions, now(), 17), 90);
    }

    #[test]
    fn weekday_averages_basic() {
        // Monday 2026-08-31 and Monday 2026-09-07, Wednesday 2026-09-02.
        let sessions = vec![
            sess(1, "2026-08-31 09:00", Some("2026-08-31 11:00"), None),
            sess(1, "2026-09-07 09:00", Some("2026-09-07 13:00"), None),
            sess(1, "2026-09-02 09:00", Some("2026-09-02 12:00"), None),
        ];
        let averages = weekday_averages(&sessions, now());
        assert_eq!(averages.len(), 7);
        assert_eq!(averages[0].weekday, "Monday");
        assert_eq!(averages[0].average_hours, 3.0); // (2 + 4) / 2 distinct Mondays
        assert_eq!(averages[2].weekday, "Wednesday");
        assert_eq!(averages[2].average_hours, 3.0); // 3 / 1
        assert_eq!(averages[1].average_hours, 0.0); // Tuesday, no data
    }

    fn base_report(tasks: Option<Vec<TaskEntry>>) -> DetailReport {
        DetailReport {
            name: "proj".to_string(),
            description: None,
            date: "2026-09-04".to_string(),
            total_hours: 1.5,
            total_hhmm: "01:30".to_string(),
            messages: Some("- flat message".to_string()),
            tasks,
        }
    }

    #[test]
    fn format_detail_report_renders_flat_messages_when_tasks_is_none() {
        let out = format_detail_report(&base_report(None));
        assert!(out.contains("messages:\n- flat message\n"));
        assert!(!out.contains("tasks:"));
    }

    #[test]
    fn format_detail_report_renders_empty_tasks_as_empty_list() {
        let out = format_detail_report(&base_report(Some(Vec::new())));
        assert!(out.contains("tasks: []\n"));
        assert!(!out.contains("messages:"));
    }

    #[test]
    fn format_detail_report_renders_per_task_breakdown_and_drops_flat_messages() {
        let tasks = vec![
            TaskEntry {
                name: "task1".to_string(),
                status: "wip".to_string(),
                messages: Some("- message1\n- message2".to_string()),
            },
            TaskEntry {
                name: "task2".to_string(),
                status: "done".to_string(),
                messages: None,
            },
        ];
        let out = format_detail_report(&base_report(Some(tasks)));
        assert!(!out.contains("messages: null"));
        assert!(!out.contains("- flat message"));
        assert_eq!(
            out,
            "name: proj\ndate: 2026-09-04\ntotal_hours: 1.5\ntotal_hhmm: 01:30\n\
             tasks:\n- task1: wip\n  messages:\n  - message1\n  - message2\n- task2: done\n"
        );
    }

    fn summary(name: &str, tasks: &[(&str, &str)]) -> ProjectSummary {
        ProjectSummary {
            name: name.to_string(),
            tasks: tasks
                .iter()
                .map(|(name, status)| TaskSummary {
                    name: name.to_string(),
                    status: status.to_string(),
                })
                .collect(),
        }
    }

    #[test]
    fn organization_report_nests_tasks_under_their_project() {
        let report = OrganizationReport {
            name: "acme".to_string(),
            description: None,
            projects: vec![
                summary("alpha", &[("one", "wip"), ("two", "done")]),
                summary("beta", &[("three", "queue")]),
            ],
        };
        assert_eq!(
            format_organization_report(&report),
            "name: acme\n\
             projects:\n\
             - alpha\n\
             \x20   - one: wip\n\
             \x20   - two: done\n\
             - beta\n\
             \x20   - three: queue\n"
        );
    }

    #[test]
    fn organization_report_lists_a_project_with_no_tasks() {
        let report = OrganizationReport {
            name: "acme".to_string(),
            description: None,
            projects: vec![summary("empty", &[])],
        };
        assert!(format_organization_report(&report).ends_with("projects:\n- empty\n"));
    }

    #[test]
    fn organization_report_with_no_projects_is_an_empty_list() {
        let report = OrganizationReport {
            name: "acme".to_string(),
            description: Some("notes".to_string()),
            projects: Vec::new(),
        };
        let out = format_organization_report(&report);
        assert!(out.contains("description: notes\n"), "{out}");
        assert!(out.ends_with("projects: []\n"), "{out}");
    }

    #[test]
    fn weekday_averages_no_data_is_zero_filled() {
        let averages = weekday_averages(&[], now());
        assert_eq!(averages.len(), 7);
        assert!(averages.iter().all(|a| a.average_hours == 0.0));
        assert_eq!(averages[0].weekday, "Monday");
        assert_eq!(averages[6].weekday, "Sunday");
    }
}
