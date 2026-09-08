use crate::args::Format;
use crate::error::Result;
use crate::models::Session;
use chrono::{Datelike, NaiveDate, NaiveDateTime, Weekday};
use serde::Serialize;
use serde_yaml::{Mapping, Value};
use std::collections::{HashMap, HashSet};

/// What a text report says where a session table would otherwise go.
const NO_SESSIONS: &str = "_No sessions in this period._";

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

// ---- the period a report covers ---------------------------------------

/// The days an `info` report covers. Either a single day (`--date`,
/// defaulting to today) or an interval (`--from`/`--to`) -- the CLI lets
/// you write one formulation or the other, never both. `from: None` is an
/// open start: every session up to and including `to`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateRange {
    pub from: Option<NaiveDate>,
    pub to: NaiveDate,
}

impl DateRange {
    pub fn day(date: NaiveDate) -> Self {
        DateRange {
            from: Some(date),
            to: date,
        }
    }

    /// The one date this range covers, when it covers exactly one -- what
    /// decides between a `date:` and a `from:`/`to:` header, and whether
    /// session tables need a `Date` column at all.
    pub fn single_day(&self) -> Option<NaiveDate> {
        self.from.filter(|from| *from == self.to)
    }

    pub fn contains(&self, date: NaiveDate) -> bool {
        date <= self.to && self.from.map(|from| date >= from).unwrap_or(true)
    }

    /// How the period reads in a text report's summary line.
    pub fn label(&self) -> String {
        match (self.single_day(), self.from) {
            (Some(day), _) => fmt_date(day),
            (None, Some(from)) => format!("{} to {}", fmt_date(from), fmt_date(self.to)),
            (None, None) => format!("up to {}", fmt_date(self.to)),
        }
    }
}

fn fmt_date(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

/// The subset of `sessions` that started within `range`, oldest first --
/// the date filter every `info` report applies before summing, and the
/// ordering every session table is printed in.
pub fn in_range(sessions: Vec<Session>, range: DateRange) -> Vec<Session> {
    let mut kept: Vec<Session> = sessions
        .into_iter()
        .filter(|s| range.contains(s.start.date()))
        .collect();
    kept.sort_by_key(|s| s.start);
    kept
}

pub fn minutes_to_hhmm(total_minutes: i64) -> String {
    format!("{:02}:{:02}", total_minutes / 60, total_minutes % 60)
}

pub fn round_to_half_hour(hours: f64) -> f64 {
    (hours * 2.0).round() / 2.0
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

// ---- report shapes -----------------------------------------------------

/// A model's own YAML fields minus the two every report already prints for
/// itself (`id`, `name`) -- i.e. its settings, in declaration order, for
/// the front matter block at the top of a text report. Taken
/// straight off `Serialize` so a field added to `Project`/`Task`/
/// `Organization` shows up here without a second list to keep in sync.
pub fn settings_of<T: Serialize>(item: &T) -> Result<Mapping> {
    let map = match serde_yaml::to_value(item)? {
        Value::Mapping(map) => map,
        _ => Mapping::new(),
    };
    // Rebuilt rather than `Mapping::remove`d from: that's a swap-remove,
    // which would shuffle the field a struct declared last into the hole it
    // left and lose the declaration order this is printed in.
    Ok(map
        .into_iter()
        .filter(|(key, _)| !matches!(key.as_str(), Some("id" | "name")))
        .collect())
}

/// The scalars every `info` report shares: what it's about, the period it
/// covers, the time that went into it, and the underlying row's settings.
/// Serialized on its own it's the YAML front matter of a text report; it's
/// `#[serde(flatten)]`ed into the YAML of the full report so both formats
/// carry the same fields under the same names.
#[derive(Debug, Clone, Serialize)]
pub struct Header {
    pub name: String,
    /// Set only for a single-day report; `from`/`to` are set only for an
    /// interval, so the header says which formulation was asked for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    pub total_hours: f64,
    pub total_hhmm: String,
    #[serde(skip_serializing_if = "Mapping::is_empty")]
    pub settings: Mapping,
    /// The markdown notes off the row. Kept out of a text report's front
    /// matter (it's the body below the divider instead) but present in the
    /// YAML, where there is no body.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip)]
    pub period: DateRange,
}

impl Header {
    pub fn new(
        name: String,
        description: &str,
        settings: Mapping,
        period: DateRange,
        total_minutes: i64,
    ) -> Self {
        let (date, from, to) = match period.single_day() {
            Some(day) => (Some(fmt_date(day)), None, None),
            None => (None, period.from.map(fmt_date), Some(fmt_date(period.to))),
        };
        Header {
            name,
            date,
            from,
            to,
            total_hours: round_to_half_hour(total_minutes as f64 / 60.0),
            total_hhmm: minutes_to_hhmm(total_minutes),
            settings,
            description: non_empty(description),
            period,
        }
    }
}

/// One row of a session table: when the stretch of work ran, how long it
/// lasted, and the note left when it was closed. `end: None` means the
/// session is still open, and `duration` is measured against now.
#[derive(Debug, Serialize)]
pub struct SessionRow {
    /// Only carried when the report spans more than one day -- on a
    /// single-day report every row would repeat the header's own date.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    pub start: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,
    pub duration: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Turns `sessions` (already filtered to the report's period) into table
/// rows. `dated` comes from the period covering more than one day.
pub fn session_rows(sessions: &[Session], now: NaiveDateTime, dated: bool) -> Vec<SessionRow> {
    sessions
        .iter()
        .map(|s| SessionRow {
            date: dated.then(|| fmt_date(s.start.date())),
            start: s.start.format("%H:%M").to_string(),
            end: s.end.map(|e| e.format("%H:%M").to_string()),
            duration: minutes_to_hhmm(s.duration_minutes(now)),
            message: s.message.as_deref().and_then(non_empty),
        })
        .collect()
}

/// One task's section of a project or organization report: what it is,
/// where it stands, and every session it had in the period.
#[derive(Debug, Serialize)]
pub struct TaskReport {
    pub name: String,
    pub status: String,
    /// The status spelled out for a heading -- `wip` reads as "work in
    /// progress" once it's sitting next to a task name.
    pub status_label: String,
    pub total_hours: f64,
    pub total_hhmm: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub sessions: Vec<SessionRow>,
}

/// One project's section of an organization report. `total_hhmm` is the
/// union of its tasks' sessions, so it can be less than their totals summed
/// -- see `merged_total_minutes`.
#[derive(Debug, Serialize)]
pub struct ProjectReport {
    pub name: String,
    pub total_hours: f64,
    pub total_hhmm: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub tasks: Vec<TaskReport>,
}

#[derive(Debug, Serialize)]
pub struct TaskInfo {
    #[serde(flatten)]
    pub header: Header,
    pub sessions: Vec<SessionRow>,
}

#[derive(Debug, Serialize)]
pub struct ProjectInfo {
    #[serde(flatten)]
    pub header: Header,
    pub tasks: Vec<TaskReport>,
}

#[derive(Debug, Serialize)]
pub struct OrganizationInfo {
    #[serde(flatten)]
    pub header: Header,
    pub projects: Vec<ProjectReport>,
}

#[derive(Debug, Serialize)]
pub struct WeekdayReport {
    pub name: String,
    pub weekdays: Vec<WeekdayAverage>,
}

// ---- rendering ---------------------------------------------------------

pub fn render_task_info(info: &TaskInfo, format: Format) -> Result<String> {
    if let Format::Yaml = format {
        return Ok(serde_yaml::to_string(info)?);
    }
    let mut md = Md::new();
    md.heading(1, &info.header.name);
    md.block(&summary_line(&info.header));
    if let Some(description) = &info.header.description {
        md.block(description);
    }
    md.session_table(&info.sessions);
    document(&info.header, md)
}

pub fn render_project_info(info: &ProjectInfo, format: Format) -> Result<String> {
    if let Format::Yaml = format {
        return Ok(serde_yaml::to_string(info)?);
    }
    let mut md = Md::new();
    md.heading(1, &info.header.name);
    md.block(&summary_line(&info.header));
    if let Some(description) = &info.header.description {
        md.block(description);
    }
    md.task_summary(&info.tasks);
    for task in &info.tasks {
        md.task_section(task, 2);
    }
    document(&info.header, md)
}

pub fn render_organization_info(info: &OrganizationInfo, format: Format) -> Result<String> {
    if let Format::Yaml = format {
        return Ok(serde_yaml::to_string(info)?);
    }
    let mut md = Md::new();
    md.heading(1, &info.header.name);
    md.block(&summary_line(&info.header));
    if let Some(description) = &info.header.description {
        md.block(description);
    }
    if info.projects.is_empty() {
        md.block(NO_SESSIONS);
    }
    md.table(
        &["Project", "Time"],
        &info
            .projects
            .iter()
            .map(|p| vec![cell(&p.name), p.total_hhmm.clone()])
            .collect::<Vec<_>>(),
    );
    for project in &info.projects {
        md.heading(2, &project.name);
        md.block(&spent(&project.total_hhmm, project.total_hours));
        if let Some(description) = &project.description {
            md.block(description);
        }
        md.task_summary(&project.tasks);
        for task in &project.tasks {
            md.task_section(task, 3);
        }
    }
    document(&info.header, md)
}

/// A text report: the header's scalars as fenced YAML front matter, then
/// the markdown body -- the same shape `md_edit` opens a
/// project/task/organization in, so a report reads like the thing it
/// reports on
fn document(header: &Header, md: Md) -> Result<String> {
    let mut front = header.clone();
    front.description = None; // it's the body below the front matter instead
    let mut out = String::from("---\n");
    out.push_str(&serde_yaml::to_string(&front)?);
    out.push_str("---\n\n");
    out.push_str(&md.out);
    Ok(out)
}

/// The line under a report's title: the period it covers and the time in
/// it. The period goes here rather than in the heading so the title stays
/// just the name of the thing.
fn summary_line(header: &Header) -> String {
    format!(
        "{} -- {}",
        header.period.label(),
        spent(&header.total_hhmm, header.total_hours)
    )
}

fn spent(hhmm: &str, hours: f64) -> String {
    format!("{hhmm} ({hours:.1} h)")
}

fn non_empty(text: &str) -> Option<String> {
    Some(text.trim())
        .filter(|t| !t.is_empty())
        .map(str::to_string)
}

/// A markdown cell: pipes escaped and newlines flattened, so a typed note
/// can't break the table it's printed in.
fn cell(text: &str) -> String {
    text.replace('|', "\\|")
        .replace(['\n', '\r'], " ")
        .trim()
        .to_string()
}

/// The messages recorded on `rows`' closing events, as a bulleted,
/// copy-paste-ready list.
pub fn message_bullets(rows: &[SessionRow]) -> Option<String> {
    let messages: Vec<String> = rows
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

/// A markdown buffer that keeps exactly one blank line between blocks, so
/// the callers above can just append sections without tracking spacing.
struct Md {
    out: String,
}

impl Md {
    fn new() -> Self {
        Md { out: String::new() }
    }

    fn block(&mut self, text: &str) {
        if text.trim().is_empty() {
            return;
        }
        if !self.out.is_empty() {
            self.out.push('\n');
        }
        self.out.push_str(text.trim_end_matches('\n'));
        self.out.push('\n');
    }

    fn heading(&mut self, level: usize, text: &str) {
        self.block(&format!("{} {}", "#".repeat(level), text));
    }

    /// A pipe table with every column padded to its widest cell -- the
    /// point of the text format is being read as-is in a terminal, where
    /// ragged columns are the hard part.
    fn table(&mut self, headers: &[&str], rows: &[Vec<String>]) {
        if rows.is_empty() {
            return;
        }
        let widths: Vec<usize> = headers
            .iter()
            .enumerate()
            .map(|(i, h)| {
                rows.iter()
                    .filter_map(|r| r.get(i))
                    .map(|c| c.chars().count())
                    .chain(std::iter::once(h.chars().count()))
                    .max()
                    .unwrap_or(0)
            })
            .collect();

        let line = |cells: &[String]| {
            let padded: Vec<String> = widths
                .iter()
                .enumerate()
                .map(|(i, w)| {
                    format!(
                        "{:<w$}",
                        cells.get(i).map(String::as_str).unwrap_or(""),
                        w = w
                    )
                })
                .collect();
            format!("| {} |", padded.join(" | "))
        };

        let mut table = line(&headers.iter().map(|h| h.to_string()).collect::<Vec<_>>());
        table.push_str(&format!(
            "\n|{}|",
            widths
                .iter()
                .map(|w| "-".repeat(w + 2))
                .collect::<Vec<_>>()
                .join("|")
        ));
        for row in rows {
            table.push('\n');
            table.push_str(&line(row));
        }
        self.block(&table);
    }

    /// The "how much time went into each task" table under a project.
    fn task_summary(&mut self, tasks: &[TaskReport]) {
        if tasks.is_empty() {
            self.block(NO_SESSIONS);
            return;
        }
        self.table(
            &["Task", "Status", "Time"],
            &tasks
                .iter()
                .map(|t| vec![cell(&t.name), t.status_label.clone(), t.total_hhmm.clone()])
                .collect::<Vec<_>>(),
        );
    }

    fn task_section(&mut self, task: &TaskReport, level: usize) {
        self.heading(level, &format!("{} -- {}", task.name, task.status_label));
        self.block(&spent(&task.total_hhmm, task.total_hours));
        if let Some(description) = &task.description {
            self.block(description);
        }
        if let Some(messages) = message_bullets(&task.sessions) {
            self.block(&messages);
        }
        self.session_table(&task.sessions);
    }

    fn session_table(&mut self, rows: &[SessionRow]) {
        if rows.is_empty() {
            self.block(NO_SESSIONS);
            return;
        }
        // A report over one day would repeat that date on every row, so
        // `session_rows` leaves it off and the column goes with it.
        let dated = rows.iter().any(|r| r.date.is_some());
        let mut headers: Vec<&str> = Vec::new();
        if dated {
            headers.push("Date");
        }
        headers.extend(["Start", "End", "Duration", "Message"]);

        let body: Vec<Vec<String>> = rows
            .iter()
            .map(|r| {
                let mut row = Vec::new();
                if dated {
                    row.push(r.date.clone().unwrap_or_default());
                }
                row.push(r.start.clone());
                row.push(r.end.clone().unwrap_or_else(|| "--".to_string()));
                row.push(r.duration.clone());
                row.push(match (&r.message, &r.end) {
                    (Some(m), _) => cell(m),
                    (None, None) => "(ongoing)".to_string(),
                    (None, Some(_)) => String::new(),
                });
                row
            })
            .collect();
        self.table(&headers, &body);
    }
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

    fn day(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .unwrap_or_else(|e| panic!("bad test fixture date '{s}': {e}"))
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

    // ---- the period ----------------------------------------------------

    #[test]
    fn a_single_day_range_reports_that_day() {
        let range = DateRange::day(day("2026-09-04"));
        assert_eq!(range.single_day(), Some(day("2026-09-04")));
        assert_eq!(range.label(), "2026-09-04");
        assert!(range.contains(day("2026-09-04")));
        assert!(!range.contains(day("2026-09-03")));
        assert!(!range.contains(day("2026-09-05")));
    }

    #[test]
    fn a_bounded_interval_is_inclusive_at_both_ends() {
        let range = DateRange {
            from: Some(day("2026-09-01")),
            to: day("2026-09-04"),
        };
        assert_eq!(range.single_day(), None);
        assert_eq!(range.label(), "2026-09-01 to 2026-09-04");
        assert!(range.contains(day("2026-09-01")));
        assert!(range.contains(day("2026-09-04")));
        assert!(!range.contains(day("2026-08-31")));
        assert!(!range.contains(day("2026-09-05")));
    }

    #[test]
    fn an_open_start_reaches_back_indefinitely() {
        let range = DateRange {
            from: None,
            to: day("2026-09-04"),
        };
        assert_eq!(range.single_day(), None);
        assert_eq!(range.label(), "up to 2026-09-04");
        assert!(range.contains(day("2001-01-01")));
        assert!(!range.contains(day("2026-09-05")));
    }

    #[test]
    fn in_range_filters_and_orders_oldest_first() {
        let sessions = vec![
            sess(1, "2026-09-04 13:00", Some("2026-09-04 14:00"), None),
            sess(1, "2026-09-01 09:00", Some("2026-09-01 10:00"), None),
            sess(1, "2026-09-04 09:00", Some("2026-09-04 10:00"), None),
        ];
        let kept = in_range(sessions, DateRange::day(day("2026-09-04")));
        assert_eq!(kept.len(), 2);
        assert_eq!(kept[0].start, dt("2026-09-04 09:00"));
        assert_eq!(kept[1].start, dt("2026-09-04 13:00"));
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
        TaskReport {
            name: name.to_string(),
            status: "wip".to_string(),
            status_label: "work in progress".to_string(),
            total_hours: round_to_half_hour(minutes as f64 / 60.0),
            total_hhmm: minutes_to_hhmm(minutes),
            description: Some("Task notes.".to_string()),
            sessions,
        }
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

    #[test]
    fn a_text_report_is_front_matter_then_a_markdown_body() {
        let info = TaskInfo {
            header: Header::new(
                "proj/task".to_string(),
                "Task notes.",
                settings(&[("status", "wip")]),
                DateRange::day(day("2026-09-04")),
                120,
            ),
            sessions: one_row(),
        };
        assert_eq!(
            render_task_info(&info, Format::Text).unwrap(),
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
        let out = render_task_info(&info, Format::Text).unwrap();
        let front = out.strip_prefix("---\n").expect("front matter");
        let (front, body) = front.split_once("\n---\n").expect("a closing divider");
        assert!(!front.contains("Task notes."), "{front}");
        assert!(body.contains("Task notes."), "{body}");
        // Nothing logged that day, so there's no table to print.
        assert!(body.contains(NO_SESSIONS), "{body}");
    }

    #[test]
    fn a_yaml_report_carries_the_same_fields_flat() {
        let info = TaskInfo {
            header: Header::new(
                "proj/task".to_string(),
                "Task notes.",
                settings(&[("status", "wip")]),
                DateRange::day(day("2026-09-04")),
                120,
            ),
            sessions: one_row(),
        };
        let out = render_task_info(&info, Format::Yaml).unwrap();
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
        let out = render_task_info(&info, Format::Text).unwrap();
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
        let out = render_task_info(&info, Format::Text).unwrap();
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
        let out = render_project_info(&info, Format::Text).unwrap();
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
                name: "proj".to_string(),
                total_hours: 2.0,
                total_hhmm: "02:00".to_string(),
                description: Some("Project notes.".to_string()),
                tasks: vec![task_report("task1", 120, one_row())],
            }],
        };
        let out = render_organization_info(&info, Format::Text).unwrap();
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
        let out = render_organization_info(&info, Format::Text).unwrap();
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
        let out = render_task_info(&info, Format::Text).unwrap();
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
}
