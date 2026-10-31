//! Building a worklog: one entry per day, one line per task.

use super::*;

// ---- the worklog -------------------------------------------------------

/// A task name on one line, as a worklog or a table cell needs it.
pub(super) fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// What was done on a task: `Completed <task>` once it is done, `Worked on
/// <task>` while it is not.
pub fn worklog_line(name: &str, completed: bool) -> String {
    let verb = if completed { "Completed" } else { "Worked on" };
    format!("{verb} {}", one_line(name))
}

/// One line per task, in report order: the `worklog` format. A task is
/// listed because it had a session in the period, so `done` here means
/// finished at some point and worked on in this period -- tasks have no
/// completion date to be more exact.
pub fn worklog_text<'a>(tasks: impl Iterator<Item = &'a TaskReport>) -> String {
    tasks
        .map(|t| worklog_line(&t.name, t.status == TaskStatus::Done) + "\n")
        .collect()
}

/// One day of a [`Worklog`]: the shape the time platform receives.
#[derive(Debug, Clone, PartialEq, Serialize, serde::Deserialize)]
pub struct WorklogEntry {
    pub project: String,
    pub activity: String,
    /// `yyyy-mm-dd`
    pub day: String,
    /// Hours, as a decimal (`1.5` is an hour and a half).
    pub time: f64,
    /// What was done, one `- ` line per task.
    pub note: String,
}

/// The hours of a period as one entry per day. Entries on one day are never
/// split: everything done that day is accumulated into a single entry.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Worklog {
    pub entries: Vec<WorklogEntry>,
    /// Whether a session is still open, so its figure is measured against
    /// now and a later push may differ. Not sent.
    #[serde(skip)]
    pub open: bool,
}

impl Worklog {
    /// Builds the worklog of `projects` (as `project_reports` returned
    /// them) over `sessions` (the union it returned with them). Each day's
    /// hours are the merged total of that day's sessions, so tasks worked
    /// in parallel are not counted twice and the entries add up to what
    /// `info` reports. A done task reads `Completed` on the last day it was
    /// worked on in the period and `Worked on` before that.
    pub fn build(
        projects: &[ProjectReport],
        sessions: &[Session],
        now: NaiveDateTime,
        gap_minutes: i64,
        project: &str,
        activity: &str,
    ) -> Self {
        let mut by_day: std::collections::BTreeMap<NaiveDate, Vec<Session>> = Default::default();
        for s in sessions {
            by_day.entry(s.start.date()).or_default().push(s.clone());
        }
        let tasks: Vec<&TaskReport> = projects.iter().flat_map(|p| &p.tasks).collect();
        let entries = by_day
            .into_iter()
            .map(|(day, of_day)| {
                let lines: Vec<String> = tasks
                    .iter()
                    .filter(|t| t.sessions.iter().any(|r| r.day == day))
                    .map(|t| {
                        let last = t.sessions.iter().map(|r| r.day).max() == Some(day);
                        let done = last && t.status == TaskStatus::Done;
                        format!("- {}", worklog_line(&t.name, done))
                    })
                    .collect();
                let minutes = merged_total_minutes(&of_day, now, gap_minutes);
                WorklogEntry {
                    project: project.to_string(),
                    activity: activity.to_string(),
                    day: fmt_date(day),
                    time: (minutes as f64 / 60.0 * 100.0).round() / 100.0,
                    note: lines.join("\n"),
                }
            })
            .collect();
        Worklog {
            entries,
            open: sessions.iter().any(Session::is_ongoing),
        }
    }

    pub fn total_hours(&self) -> f64 {
        self.entries.iter().map(|e| e.time).sum()
    }

    /// The request body: `{"entries": [...]}`.
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(self).map_err(|e| {
            crate::error::IterError::CommandFailed(format!("could not encode the worklog: {e}"))
        })
    }
}
