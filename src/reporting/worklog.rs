//! Building a worklog: one entry per day, one line per task.

use super::*;

pub(super) fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `Completed <task>` once the task is done, else `Worked on <task>`.
pub fn worklog_line(name: &str, completed: bool) -> String {
    let verb = if completed { "Completed" } else { "Worked on" };
    format!("{verb} {}", one_line(name))
}

/// One line per task, in report order (the `worklog` format); `done` means finished at some point and worked on in the period: tasks have no completion date.
pub fn worklog_text<'a>(tasks: impl Iterator<Item = &'a TaskReport>) -> String {
    tasks
        .map(|t| worklog_line(&t.name, t.status == TaskStatus::Done) + "\n")
        .collect()
}

#[derive(Debug, Clone, PartialEq, Serialize, serde::Deserialize)]
pub struct WorklogEntry {
    pub project: String,
    pub activity: String,
    pub day: String,
    pub time: f64,
    pub note: String,
}

/// The hours of a period as one entry per day; a day's work is never split. Each day's hours are
/// rounded to the nearest half hour (so the total is the sum of rounded days, not a rounded sum);
/// a day that rounds to nothing is left out.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Worklog {
    pub entries: Vec<WorklogEntry>,
    /// Whether a session is still open, so a later push may differ. Not sent.
    #[serde(skip)]
    pub open: bool,
}

impl Worklog {
    /// Builds the worklog of `projects` over `sessions` (both from `project_reports`). A day's hours are the merged total of its sessions (a pause shorter than `gap_minutes` counts as work), so parallel tasks are not double-counted, rounded to the nearest half hour.
    /// A done task reads `Completed` on the last day it was worked on, `Worked on` before.
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
            .filter_map(|(day, of_day)| {
                let minutes = merged_total_minutes(&of_day, now, gap_minutes);
                let time = round_to_half_hour(minutes as f64 / 60.0);
                if time == 0.0 {
                    return None;
                }
                let lines: Vec<String> = tasks
                    .iter()
                    .filter(|t| t.sessions.iter().any(|r| r.day == day))
                    .map(|t| {
                        let last = t.sessions.iter().map(|r| r.day).max() == Some(day);
                        let done = last && t.status == TaskStatus::Done;
                        format!("- {}", worklog_line(&t.name, done))
                    })
                    .collect();
                Some(WorklogEntry {
                    project: project.to_string(),
                    activity: activity.to_string(),
                    day: fmt_date(day),
                    time,
                    note: lines.join("\n"),
                })
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

    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(self).map_err(|e| {
            crate::error::IterError::CommandFailed(format!("could not encode the worklog: {e}"))
        })
    }
}
