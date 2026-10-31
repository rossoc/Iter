use super::*;
use crate::fixtures::{self, day, dt};
use chrono::NaiveDate;

fn now() -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 9, 4)
        .expect("2026-09-04 is a valid calendar date")
        .and_hms_opt(12, 0, 0)
        .expect("12:00:00 is a valid time")
}

fn sess(task_id: i64, start: &str, end: Option<&str>, message: Option<&str>) -> Session {
    Session {
        message: message.map(|m| m.to_string()),
        ..fixtures::session(task_id, dt(start), end.map(dt))
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

// ---- the period ----------------------------------------------------

#[test]
fn a_single_day_range_reports_that_day() {
    let range = DateRange::day(day("2026-09-04"));
    assert_eq!(range.single_day(), Some(day("2026-09-04")));
    assert_eq!(range.label(), "2026-09-04");
}

#[test]
fn a_bounded_interval_is_inclusive_at_both_ends() {
    let range = DateRange {
        from: Some(day("2026-09-01")),
        to: day("2026-09-04"),
    };
    assert_eq!(range.single_day(), None);
    assert_eq!(range.label(), "2026-09-01 to 2026-09-04");
}

#[test]
fn an_open_start_reaches_back_indefinitely() {
    let range = DateRange {
        from: None,
        to: day("2026-09-04"),
    };
    assert_eq!(range.single_day(), None);
    assert_eq!(range.label(), "up to 2026-09-04");
}

// ---- totals --------------------------------------------------------

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

#[test]
fn weekday_averages_no_data_is_zero_filled() {
    let averages = weekday_averages(&[], now());
    assert_eq!(averages.len(), 7);
    assert!(averages.iter().all(|a| a.average_hours == 0.0));
    assert_eq!(averages[0].weekday, "Monday");
    assert_eq!(averages[6].weekday, "Sunday");
}

// ---- rows ----------------------------------------------------------

#[test]
fn session_rows_omit_the_date_on_a_single_day_report() {
    let sessions = vec![sess(
        1,
        "2026-09-04 09:00",
        Some("2026-09-04 10:30"),
        Some("did a thing"),
    )];
    let rows = session_rows(&sessions, now(), false);
    assert_eq!(rows[0].date, None);
    assert_eq!(rows[0].start, "09:00");
    assert_eq!(rows[0].end.as_deref(), Some("10:30"));
    assert_eq!(rows[0].duration, "01:30");
    assert_eq!(rows[0].message.as_deref(), Some("did a thing"));
}

#[test]
fn session_rows_carry_the_date_when_the_period_spans_days() {
    let sessions = vec![sess(1, "2026-09-04 09:00", None, None)];
    let rows = session_rows(&sessions, now(), true);
    assert_eq!(rows[0].date.as_deref(), Some("2026-09-04"));
    // Still open, so it's measured against now() -- 09:00 to 12:00.
    assert_eq!(rows[0].end, None);
    assert_eq!(rows[0].duration, "03:00");
}

#[test]
fn message_bullets_joins_present_messages_and_skips_none() {
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
    let rows = session_rows(&sessions, now(), false);
    assert_eq!(
        message_bullets(&rows),
        Some("- first\n- second".to_string())
    );
}

#[test]
fn message_bullets_is_none_when_no_session_has_one() {
    let rows = session_rows(
        &[sess(1, "2026-09-01 09:00", Some("2026-09-01 10:00"), None)],
        now(),
        false,
    );
    assert_eq!(message_bullets(&rows), None);
}

#[test]
fn message_bullets_does_not_double_an_existing_leading_dash() {
    let rows = session_rows(
        &[sess(
            1,
            "2026-09-01 09:00",
            Some("2026-09-01 10:00"),
            Some("- already bulleted"),
        )],
        now(),
        false,
    );
    assert_eq!(
        message_bullets(&rows),
        Some("- already bulleted".to_string())
    );
}

// ---- rendering -----------------------------------------------------

fn settings(pairs: &[(&str, &str)]) -> Mapping {
    let mut map = Mapping::new();
    for (k, v) in pairs {
        map.insert(Value::from(*k), Value::from(*v));
    }
    map
}

fn task_report(name: &str, minutes: i64, sessions: Vec<SessionRow>) -> TaskReport {
    TaskReport::new(
        0,
        name.to_string(),
        TaskStatus::Wip,
        minutes,
        Some("Task notes.".to_string()),
        sessions,
    )
}

fn one_row() -> Vec<SessionRow> {
    session_rows(
        &[sess(
            1,
            "2026-09-04 09:00",
            Some("2026-09-04 11:00"),
            Some("wrote the docs"),
        )],
        now(),
        false,
    )
}

/// The one-task report the text and YAML renderings are checked against.
fn sample_info() -> TaskInfo {
    TaskInfo {
        header: Header::new(
            "proj/task".to_string(),
            "Task notes.",
            settings(&[("status", "wip")]),
            DateRange::day(day("2026-09-04")),
            120,
        ),
        sessions: one_row(),
    }
}

#[test]
fn a_text_report_is_front_matter_then_a_markdown_body() {
    let info = sample_info();
    assert_eq!(
        info.render(Format::Text).unwrap(),
        "---\n\
         name: proj/task\n\
         date: 2026-09-04\n\
         total_hours: 2.0\n\
         total_hhmm: 02:00\n\
         settings:\n  status: wip\n\
         ---\n\
         \n# proj/task\n\
         \n2026-09-04 -- 02:00 (2.0 h)\n\
         \nTask notes.\n\
         \n| Start | End   | Duration | Message        |\n\
         |-------|-------|----------|----------------|\n\
         | 09:00 | 11:00 | 02:00    | wrote the docs |\n"
    );
}

#[test]
fn the_description_is_the_body_not_a_front_matter_field() {
    let info = TaskInfo {
        header: Header::new(
            "proj/task".to_string(),
            "Task notes.",
            Mapping::new(),
            DateRange::day(day("2026-09-04")),
            0,
        ),
        sessions: Vec::new(),
    };
    let out = info.render(Format::Text).unwrap();
    let front = out.strip_prefix("---\n").expect("front matter");
    let (front, body) = front.split_once("\n---\n").expect("a closing divider");
    assert!(!front.contains("Task notes."), "{front}");
    assert!(body.contains("Task notes."), "{body}");
    // Nothing logged that day, so there's no table to print.
    assert!(body.contains(NO_SESSIONS), "{body}");
}

/// An open start -- `--to` with no `--from` -- leaves `from:` out
/// altogether rather than emitting a null.
#[test]
fn an_open_ended_range_omits_from() {
    let info = TaskInfo {
        header: Header::new(
            "proj".to_string(),
            "",
            Mapping::new(),
            DateRange {
                from: None,
                to: day("2026-09-04"),
            },
            0,
        ),
        sessions: Vec::new(),
    };
    let out = info.render(Format::Yaml).unwrap();
    assert!(!out.contains("from:"), "{out}");
    assert!(out.contains("to: 2026-09-04\n"), "{out}");
}

#[test]
fn a_yaml_report_carries_the_same_fields_flat() {
    let info = sample_info();
    let out = info.render(Format::Yaml).unwrap();
    // The header is flattened, not nested under a `header:` key.
    assert!(out.starts_with("name: proj/task\n"), "{out}");
    assert!(out.contains("date: 2026-09-04\n"), "{out}");
    assert!(out.contains("description: Task notes.\n"), "{out}");
    assert!(out.contains("message: wrote the docs\n"), "{out}");
    assert!(!out.contains("header:"), "{out}");
    // ...and it parses back as YAML, which the hand-rolled renderer it
    // replaced could not promise for a message starting with `-`.
    serde_yaml::from_str::<serde_yaml::Value>(&out).expect("valid yaml");
}

#[test]
fn an_interval_report_says_from_and_to_and_dates_every_row() {
    let range = DateRange {
        from: Some(day("2026-09-01")),
        to: day("2026-09-04"),
    };
    let info = TaskInfo {
        header: Header::new("proj/task".to_string(), "", Mapping::new(), range, 60),
        sessions: session_rows(
            &[sess(1, "2026-09-01 09:00", Some("2026-09-01 10:00"), None)],
            now(),
            true,
        ),
    };
    let out = info.render(Format::Text).unwrap();
    assert!(out.contains("from: 2026-09-01\nto: 2026-09-04\n"), "{out}");
    assert!(!out.contains("\ndate:"), "{out}");
    assert!(out.contains("2026-09-01 to 2026-09-04 -- 01:00"), "{out}");
    assert!(out.contains("| Date       | Start |"), "{out}");
    assert!(out.contains("| 2026-09-01 | 09:00 |"), "{out}");
}

#[test]
fn an_ongoing_session_has_no_end_and_says_so() {
    let info = TaskInfo {
        header: Header::new(
            "proj/task".to_string(),
            "",
            Mapping::new(),
            DateRange::day(day("2026-09-04")),
            180,
        ),
        sessions: session_rows(&[sess(1, "2026-09-04 09:00", None, None)], now(), false),
    };
    let out = info.render(Format::Text).unwrap();
    assert!(
        out.contains("| 09:00 | --  | 03:00    | (ongoing) |"),
        "{out}"
    );
}

#[test]
fn a_project_report_breaks_the_period_down_per_task() {
    let info = ProjectInfo {
        header: Header::new(
            "proj".to_string(),
            "Project notes.",
            settings(&[("base_path", "/tmp/proj")]),
            DateRange::day(day("2026-09-04")),
            150,
        ),
        tasks: vec![task_report("task1", 120, one_row())],
    };
    let out = info.render(Format::Text).unwrap();
    assert!(out.contains("base_path: /tmp/proj"), "{out}");
    assert!(out.contains("\n# proj\n"), "{out}");
    // The per-task summary table, then the task's own section.
    assert!(
        out.contains("| Task  | Status           | Time  |"),
        "{out}"
    );
    assert!(
        out.contains("| task1 | work in progress | 02:00 |"),
        "{out}"
    );
    assert!(out.contains("\n## task1 -- work in progress\n"), "{out}");
    assert!(out.contains("- wrote the docs\n"), "{out}");
}

#[test]
fn an_organization_report_nests_project_then_task_headings() {
    let info = OrganizationInfo {
        header: Header::new(
            "acme".to_string(),
            "Everything 4BC works on.",
            Mapping::new(),
            DateRange::day(day("2026-09-04")),
            150,
        ),
        projects: vec![ProjectReport {
            id: 0,
            name: "proj".to_string(),
            total: Total::new(120),
            description: Some("Project notes.".to_string()),
            tasks: vec![task_report("task1", 120, one_row())],
        }],
    };
    let out = info.render(Format::Text).unwrap();
    assert!(out.contains("\n# acme\n"), "{out}");
    assert!(out.contains("\n## proj\n"), "{out}");
    assert!(out.contains("\n### task1 -- work in progress\n"), "{out}");
    assert!(out.contains("| Project | Time  |"), "{out}");
}

#[test]
fn an_organization_with_nothing_logged_says_so() {
    let info = OrganizationInfo {
        header: Header::new(
            "acme".to_string(),
            "",
            Mapping::new(),
            DateRange::day(day("2026-09-04")),
            0,
        ),
        projects: Vec::new(),
    };
    let out = info.render(Format::Text).unwrap();
    assert!(out.contains(NO_SESSIONS), "{out}");
    assert!(!out.contains("| Project"), "{out}");
}

#[test]
fn a_pipe_in_a_message_cannot_break_the_table() {
    let rows = session_rows(
        &[sess(
            1,
            "2026-09-04 09:00",
            Some("2026-09-04 10:00"),
            Some("a | b\nc"),
        )],
        now(),
        false,
    );
    let info = TaskInfo {
        header: Header::new(
            "proj/task".to_string(),
            "",
            Mapping::new(),
            DateRange::day(day("2026-09-04")),
            60,
        ),
        sessions: rows,
    };
    let out = info.render(Format::Text).unwrap();
    assert!(out.contains("| a \\| b c |"), "{out}");
}

#[test]
fn settings_of_drops_the_fields_the_report_prints_itself() {
    #[derive(Serialize)]
    struct Row {
        id: i64,
        name: String,
        github: bool,
        branch_template: String,
    }
    let settings = settings_of(&Row {
        id: 3,
        name: "proj".to_string(),
        github: true,
        branch_template: "feat/{task}".to_string(),
    })
    .unwrap();
    let keys: Vec<&str> = settings.keys().filter_map(Value::as_str).collect();
    assert_eq!(keys, vec!["github", "branch_template"]);
}

// ---- the worklog ---------------------------------------------------

fn project_of(
    tasks: Vec<(&str, TaskStatus, Vec<Session>)>,
) -> (Vec<ProjectReport>, Vec<Session>) {
    let mut all = Vec::new();
    let mut reports = Vec::new();
    for (i, (name, status, sessions)) in tasks.into_iter().enumerate() {
        let minutes = merged_total_minutes(&sessions, now(), 30);
        reports.push(TaskReport::new(
            i as i64,
            name.to_string(),
            status,
            minutes,
            None,
            session_rows(&sessions, now(), true),
        ));
        all.extend(sessions);
    }
    let project = ProjectReport {
        id: 1,
        name: "p".into(),
        total: Total::new(merged_total_minutes(&all, now(), 30)),
        description: None,
        tasks: reports,
    };
    (vec![project], all)
}

#[test]
fn done_tasks_read_completed_and_the_rest_worked_on() {
    let (projects, _) = project_of(vec![
        (
            "Ship it",
            TaskStatus::Done,
            vec![sess(1, "2026-09-04 09:00", Some("2026-09-04 10:00"), None)],
        ),
        (
            "Fix bug",
            TaskStatus::Wip,
            vec![sess(2, "2026-09-04 11:00", Some("2026-09-04 12:00"), None)],
        ),
    ]);
    let text = worklog_text(projects.iter().flat_map(|p| &p.tasks));
    assert_eq!(text, "Completed Ship it\nWorked on Fix bug\n");
}

#[test]
fn an_empty_period_has_no_lines_and_no_entries() {
    let (projects, all) = project_of(vec![]);
    assert_eq!(worklog_text(projects.iter().flat_map(|p| &p.tasks)), "");
    let log = Worklog::build(&projects, &all, now(), 30, "acme", "dev");
    assert!(log.entries.is_empty());
    assert!(!log.open);
}

#[test]
fn a_newline_in_a_task_name_stays_on_one_line() {
    assert_eq!(worklog_line("a\n b | c", false), "Worked on a b | c");
}

#[test]
fn entries_are_one_per_day_with_merged_hours() {
    let (projects, all) = project_of(vec![
        (
            "A",
            TaskStatus::Done,
            vec![
                sess(1, "2026-09-03 09:00", Some("2026-09-03 10:00"), None),
                sess(1, "2026-09-04 09:00", Some("2026-09-04 10:30"), None),
            ],
        ),
        // parallel to A on the 4th: must not be double counted
        (
            "B",
            TaskStatus::Wip,
            vec![sess(2, "2026-09-04 09:30", Some("2026-09-04 10:30"), None)],
        ),
    ]);
    let log = Worklog::build(&projects, &all, now(), 30, "acme", "dev");
    assert_eq!(log.entries.len(), 2);
    assert_eq!(log.entries[0].day, "2026-09-03");
    assert_eq!(log.entries[0].time, 1.0);
    assert_eq!(log.entries[0].note, "- Worked on A");
    assert_eq!(log.entries[1].day, "2026-09-04");
    assert_eq!(log.entries[1].time, 1.5);
    assert_eq!(log.entries[1].note, "- Completed A\n- Worked on B");
    assert_eq!(log.total_hours(), 2.5);
}

#[test]
fn an_open_session_marks_the_worklog() {
    let (projects, all) = project_of(vec![(
        "A",
        TaskStatus::Wip,
        vec![sess(1, "2026-09-04 11:00", None, None)],
    )]);
    let log = Worklog::build(&projects, &all, now(), 30, "acme", "dev");
    assert!(log.open);
    assert_eq!(log.entries[0].time, 1.0);
}

#[test]
fn the_json_shape_is_an_entries_list_without_internal_fields() {
    let (projects, all) = project_of(vec![(
        "A",
        TaskStatus::Wip,
        vec![sess(1, "2026-09-04 11:00", Some("2026-09-04 12:30"), None)],
    )]);
    let json = Worklog::build(&projects, &all, now(), 30, "acme", "dev")
        .to_json()
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(
        v,
        serde_json::json!({"entries": [{
            "project": "acme", "activity": "dev", "day": "2026-09-04",
            "time": 1.5, "note": "- Worked on A"
        }]})
    );
}
